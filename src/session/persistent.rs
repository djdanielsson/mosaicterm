//! One persistent shell per tmux session, with a disposable control client.

use crate::error::{Error, Result};
use crate::pty::streams::{ControlWrite, PtyStreams};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const SESSION_PREFIX: &str = "mosaicterm-";
const SOCKET_NAME: &str = "mosaicterm";
const CAPTURE_LIMIT: usize = 128 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionHost {
    pub ssh: Option<String>,
}

impl SessionHost {
    fn command(&self, args: &[String]) -> Result<Command> {
        if let Some(host) = &self.ssh {
            if host.is_empty()
                || host.starts_with('-')
                || host
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control() || !c.is_ascii())
            {
                return Err(error("Invalid SSH host or configuration alias"));
            }
            let remote_command = ["tmux", "-L", SOCKET_NAME, "-f", "/dev/null"]
                .into_iter()
                .chain(args.iter().map(String::as_str))
                .map(quote)
                .collect::<Vec<_>>()
                .join(" ");
            let mut command = Command::new("ssh");
            command.args([
                "-T",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=8",
                "-o",
                "ServerAliveInterval=10",
                "-o",
                "ServerAliveCountMax=2",
                "--",
                host,
                &remote_command,
            ]);
            Ok(command)
        } else {
            let mut command = Command::new("tmux");
            command
                .args(["-L", SOCKET_NAME, "-f", "/dev/null"])
                .args(args);
            Ok(command)
        }
    }

    fn run(&self, args: &[String]) -> Result<Vec<u8>> {
        let output = self.command(args)?.stdin(Stdio::null()).output()?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            let message = String::from_utf8_lossy(&output.stderr);
            Err(error(message.trim()))
        }
    }

    /// List live MosaicTerm sessions without creating a shell.
    pub fn list(&self) -> Result<Vec<PersistentSession>> {
        let output = match self.run(&strings(&["list-sessions", "-F", "#{session_name}"])) {
            Err(Error::Io(error)) if is_missing(&error.to_string()) => return Ok(Vec::new()),
            result => result?,
        };
        Ok(String::from_utf8_lossy(&output)
            .lines()
            .filter(|name| valid_name(name))
            .map(|name| PersistentSession {
                host: self.clone(),
                name: name.to_owned(),
            })
            .collect())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistentSession {
    pub host: SessionHost,
    pub name: String,
}

impl PersistentSession {
    pub fn new(host: SessionHost) -> Self {
        Self {
            host,
            name: format!("{SESSION_PREFIX}{}", uuid::Uuid::new_v4().simple()),
        }
    }

    fn target(&self) -> Result<String> {
        valid_name(&self.name)
            .then(|| format!("={}", self.name))
            .ok_or_else(|| error("Invalid MosaicTerm session name"))
    }

    /// Create a detached shell. Only this operation can create a session.
    pub fn create(&self, shell: &[String], working_dir: Option<&Path>) -> Result<()> {
        self.target()?;
        let mut args = strings(&[
            "new-session",
            "-d",
            "-s",
            &self.name,
            "-e",
            "TERM=xterm-256color",
            "-e",
            "MOSAICTERM=1",
            "-e",
            "HISTCONTROL=ignorespace",
        ]);
        if let Some(path) = working_dir {
            let path = path
                .to_str()
                .ok_or_else(|| error("tmux needs a UTF-8 starting directory"))?;
            args.extend(strings(&["-c", path]));
        }
        args.push("--".to_owned());
        args.extend_from_slice(shell);
        self.host.run(&args)?;
        Ok(())
    }

    /// End this named session without affecting other shells.
    pub fn end(&self) -> Result<()> {
        let target = self.target()?;
        match self.host.run(&strings(&["kill-session", "-t", &target])) {
            Err(Error::Io(error)) if is_missing(&error.to_string()) => Ok(()),
            result => result.map(|_| ()),
        }
    }

    pub(crate) fn attach(&self, restore_output: bool) -> Result<Attachment> {
        let target = self.target()?;
        let window = format!("{}:", self.name);
        let metadata = match self.host.run(&strings(&[
            "display-message",
            "-p",
            "-t",
            &window,
            "#{session_name}\t#{pane_id}",
        ])) {
            Err(Error::Io(error)) if is_missing(&error.to_string()) => {
                return Err(Error::Io(io::Error::other(
                    "Session ended or could not be found",
                )));
            }
            result => result?,
        };
        let metadata =
            String::from_utf8(metadata).map_err(|_| error("Invalid tmux pane metadata"))?;
        let Some((name, pane)) = metadata.trim_end_matches('\n').split_once('\t') else {
            return Err(error("Session ended or could not be found"));
        };
        if name != self.name {
            return Err(error("Session ended or could not be found"));
        }
        if !pane
            .strip_prefix('%')
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(error("tmux returned an invalid pane identifier"));
        }
        let args = strings(&["-C", "attach-session", "-t", &target]);
        let mut child = self
            .host
            .command(&args)?
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().expect("piped control output");
        let mut stdin = child.stdin.take().expect("piped control input");
        let stderr = child.stderr.take().expect("piped control errors");
        let (output_tx, output_rx) = tokio::sync::mpsc::channel(256);
        let (input_tx, input_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let selected = pane.to_owned();
        std::thread::spawn(move || {
            read_control(stdout, &selected, restore_output, ready_tx, output_tx)
        });
        let metadata_format =
            "'#{@mosaicterm_idle}\t#{@mosaicterm_exit}\t#{alternate_on}\t#{pane_current_path}'";
        let mut initial = format!("display-message -p -t {pane} {metadata_format}");
        if restore_output {
            initial.push_str(&format!(" ; capture-pane -p -S -300 -t {pane}"));
        }
        if let Err(error) = writeln!(stdin, "{initial}").and_then(|_| stdin.flush()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
        let diagnostics = std::thread::spawn(move || {
            use std::io::Read;
            let mut bytes = Vec::new();
            let mut reader = BufReader::new(stderr);
            let _ = reader.by_ref().take(8192).read_to_end(&mut bytes);
            let _ = io::copy(&mut reader, &mut io::sink());
            String::from_utf8_lossy(&bytes).trim().to_owned()
        });
        let snapshot = match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(snapshot)) => snapshot,
            readiness => {
                let _ = child.kill();
                let _ = child.wait();
                let diagnostics = diagnostics.join().unwrap_or_default();
                let reason = match readiness {
                    Ok(Err(reason)) if !reason.is_empty() => reason,
                    _ if !diagnostics.is_empty() => diagnostics,
                    _ => "tmux control client disconnected before attaching".into(),
                };
                return Err(error(&reason));
            }
        };
        let target = pane.to_owned();
        std::thread::spawn(move || {
            while let Ok(request) = input_rx.recv() {
                let result = match request {
                    ControlWrite::Input(bytes) => send_keys(&mut stdin, &target, &bytes),
                    ControlWrite::Resize { rows, cols } => {
                        writeln!(stdin, "resize-window -t {target} -x {cols} -y {rows}")
                    }
                };
                if result.is_err() || stdin.flush().is_err() {
                    break;
                }
            }
        });
        let mut streams = PtyStreams::from_control_channels(output_rx, input_tx);
        streams.attach_child(Box::new(child));
        Ok(Attachment { streams, snapshot })
    }
}

pub struct SessionSnapshot {
    pub cwd: PathBuf,
    pub recent: String,
    pub idle: Option<bool>,
    pub exit_code: Option<i32>,
    pub alternate_screen: bool,
}

pub(crate) struct Attachment {
    pub streams: PtyStreams,
    pub snapshot: SessionSnapshot,
}

fn read_control(
    stdout: impl io::Read,
    selected: &str,
    restore_output: bool,
    ready: mpsc::SyncSender<std::result::Result<SessionSnapshot, String>>,
    output: tokio::sync::mpsc::Sender<Vec<u8>>,
) {
    use std::io::Read as _;
    let mut reader = BufReader::new(stdout);
    let mut ready = Some(ready);
    let mut attached = false;
    let mut response = Vec::new();
    let mut header: Option<Vec<u8>> = None;
    let mut metadata = None;
    let mut line = Vec::new();
    loop {
        line.clear();
        if !reader
            .by_ref()
            .take(512 * 1024)
            .read_until(b'\n', &mut line)
            .is_ok_and(|count| count > 0)
        {
            break;
        }
        let Some(frame) = line.strip_suffix(b"\n") else {
            break;
        };

        if let Some(sender) = ready.take() {
            if let Some(active_header) = &header {
                if frame.strip_prefix(b"%end ") == Some(active_header) {
                    header = None;
                    if restore_output && metadata.is_none() {
                        metadata = Some(parse_metadata(&response));
                    } else {
                        let snapshot = if let Some(metadata) = metadata.take() {
                            metadata.map(|mut snapshot| {
                                snapshot.recent =
                                    String::from_utf8_lossy(&response).trim_end().to_owned();
                                snapshot
                            })
                        } else {
                            parse_metadata(&response)
                        };
                        let failed = snapshot.is_err();
                        let _ = sender.send(snapshot);
                        if failed {
                            break;
                        }
                        continue;
                    }
                } else if frame.strip_prefix(b"%error ") == Some(active_header) {
                    let _ = sender.send(Err("tmux could not read the attached pane".into()));
                    break;
                } else {
                    response.extend_from_slice(&line);
                    if response.len() > CAPTURE_LIMIT {
                        let overflow = response.len() - CAPTURE_LIMIT;
                        let trim = response[overflow..]
                            .iter()
                            .position(|&byte| byte == b'\n')
                            .map_or(response.len(), |offset| overflow + offset + 1);
                        response.drain(..trim);
                    }
                }
            } else if let Some(reason) = frame.strip_prefix(b"%exit") {
                let _ = sender.send(Err(String::from_utf8_lossy(reason).trim().to_owned()));
                break;
            } else if frame.starts_with(b"%session-changed ") {
                attached = true;
            } else if attached {
                if let Some(key) = frame.strip_prefix(b"%begin ") {
                    response.clear();
                    header = Some(key.to_vec());
                }
            }
            ready = Some(sender);
            continue;
        }

        if let Some(data) = frame
            .strip_prefix(b"%output ")
            .and_then(|payload| payload.strip_prefix(selected.as_bytes()))
            .and_then(|payload| payload.strip_prefix(b" "))
        {
            if output.blocking_send(decode_output(data)).is_err() {
                break;
            }
        }
        if frame.starts_with(b"%exit") {
            break;
        }
    }
}

fn parse_metadata(bytes: &[u8]) -> std::result::Result<SessionSnapshot, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Invalid tmux pane metadata".to_owned())?;
    let mut fields = text.trim_end_matches('\n').splitn(4, '\t');
    let (Some(idle), Some(exit_code), Some(alternate), Some(cwd)) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Err("tmux session has no terminal".into());
    };
    Ok(SessionSnapshot {
        alternate_screen: alternate == "1",
        cwd: PathBuf::from(cwd),
        recent: String::new(),
        idle: match idle {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        },
        exit_code: exit_code.parse().ok(),
    })
}

fn send_keys(writer: &mut impl Write, target: &str, bytes: &[u8]) -> io::Result<()> {
    if bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        writeln!(writer, "set-option -pq -t {target} @mosaicterm_idle 0")?;
    }
    let text = String::from_utf8_lossy(bytes);
    let mut hex_open = false;
    for ch in text.chars() {
        if ch.is_ascii() {
            if !hex_open {
                write!(writer, "send-keys -t {target} -H")?;
                hex_open = true;
            }
            write!(writer, " {:02x}", ch as u8)?;
        } else {
            if hex_open {
                writeln!(writer)?;
                hex_open = false;
            }
            writeln!(writer, "send-keys -t {target} -l -- '{}'", ch)?;
        }
    }
    if hex_open {
        writeln!(writer)?;
    }
    Ok(())
}

fn decode_output(mut bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    while let Some((&first, rest)) = bytes.split_first() {
        if first == b'\\' {
            if let Some(digits) = rest.get(..3) {
                if digits[0] <= b'3' && digits.iter().all(|digit| matches!(digit, b'0'..=b'7')) {
                    result
                        .push((digits[0] - b'0') * 64 + (digits[1] - b'0') * 8 + digits[2] - b'0');
                    bytes = &rest[3..];
                    continue;
                }
            }
        }
        result.push(first);
        bytes = rest;
    }
    result
}

fn valid_name(name: &str) -> bool {
    name.strip_prefix(SESSION_PREFIX).is_some_and(|id| {
        !id.is_empty()
            && id.len() <= 100
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    })
}

fn is_missing(message: &str) -> bool {
    message.starts_with("no server running")
        || message.starts_with("can't find session")
        || message.starts_with("no sessions")
        || message.starts_with("can\x27t find window")
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|value| (*value).to_owned()).collect()
}

fn error(message: &str) -> Error {
    Error::Io(io::Error::other(if message.is_empty() {
        "tmux command failed"
    } else {
        message
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_handoff_discards_output_that_is_already_in_the_capture() {
        let bytes = b"%session-changed $0 mosaicterm-test\n%output %2 previous\n%begin 1 2 1\n1\t7\t0\t/home/test\n%end 1 2 1\n%begin 1 3 1\nprevious\n%end 1 3 1\n%output %4 other pane\n%output %2 after\\033]133;D;7\\007\n%exit\n";
        let (ready, snapshot) = mpsc::sync_channel(1);
        let (output, mut stream) = tokio::sync::mpsc::channel(4);
        read_control(&bytes[..], "%2", true, ready, output);
        let snapshot = snapshot.recv().unwrap().unwrap();
        assert_eq!(snapshot.cwd, PathBuf::from("/home/test"));
        assert_eq!(snapshot.exit_code, Some(7));
        assert_eq!(snapshot.recent, "previous");
        assert_eq!(stream.try_recv().unwrap(), b"after\x1b]133;D;7\x07");
        assert!(stream.try_recv().is_err());
    }
}
