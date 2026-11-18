//! Ordered metadata carried by the PTY's OSC stream.

use super::program_status::ProgramEvent;
use percent_encoding::percent_decode_str;
use std::path::PathBuf;

const MAX_OSC_BYTES: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum ShellMarkKind {
    Prompt,
    PromptEnd,
    OutputStart,
    Finished(Option<i32>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct ShellMark {
    pub kind: ShellMarkKind,
    /// Built-in hooks use a session token to distinguish their prompt from nested shells.
    pub token: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    Shell(ShellMark),
    Directory(PathBuf),
    Program(ProgramEvent),
    Reset,
}

#[derive(Default)]
enum State {
    #[default]
    Ground,
    Escape,
    Osc,
    OscEscape,
    OtherString,
    OtherStringEscape,
}

/// Recognizes OSC sequences across reads while leaving visible output in the original stream.
#[derive(Default)]
pub struct TerminalEventParser {
    state: State,
    payload: Vec<u8>,
}

impl TerminalEventParser {
    /// Return events in byte order with the exclusive end of each event in this read.
    pub fn observe(&mut self, bytes: &[u8]) -> Vec<(usize, TerminalEvent)> {
        let mut events = Vec::new();
        for (index, &byte) in bytes.iter().enumerate() {
            if matches!(byte, 0x18 | 0x1a) {
                self.state = State::Ground;
                continue;
            }
            self.state = match (&self.state, byte) {
                (State::Ground, 0x1b) | (State::Escape, 0x1b) => State::Escape,
                (State::Escape, b']') => {
                    self.payload.clear();
                    State::Osc
                }
                (State::Escape, b'P' | b'X' | b'^' | b'_') => State::OtherString,
                (State::Escape | State::OscEscape, b'c') => {
                    events.push((index + 1, TerminalEvent::Reset));
                    State::Ground
                }
                (State::Osc, 0x07) | (State::OscEscape, b'\\') => {
                    let framing = if byte == 0x07 { 3 } else { 4 };
                    if self.payload.len() + framing <= MAX_OSC_BYTES {
                        if let Some(event) = parse_record(&self.payload) {
                            events.push((index + 1, event));
                        }
                    }
                    State::Ground
                }
                (State::Osc, 0x1b) => State::OscEscape,
                (State::Osc, _) => {
                    if self.payload.len() < MAX_OSC_BYTES {
                        self.payload.push(byte);
                    }
                    State::Osc
                }
                (State::OscEscape, b']') => {
                    self.payload.clear();
                    State::Osc
                }
                (State::OscEscape, 0x1b) => State::Escape,
                (State::OtherString, 0x1b) | (State::OtherStringEscape, 0x1b) => {
                    State::OtherStringEscape
                }
                (State::OtherStringEscape, b'\\') => State::Ground,
                (State::OtherString | State::OtherStringEscape, _) => State::OtherString,
                _ => State::Ground,
            };
        }
        events
    }
}

fn parse_record(payload: &[u8]) -> Option<TerminalEvent> {
    let (protocol, body) = std::str::from_utf8(payload).ok()?.split_once(';')?;
    match protocol {
        "7" => parse_directory(body).map(TerminalEvent::Directory),
        "133" => parse_shell_mark(body).map(TerminalEvent::Shell),
        "7501" => ProgramEvent::parse(body).map(TerminalEvent::Program),
        _ => None,
    }
}

fn parse_shell_mark(body: &str) -> Option<ShellMark> {
    let mut parts = body.split(';');
    let kind = match parts.next()? {
        "A" => ShellMarkKind::Prompt,
        "B" => ShellMarkKind::PromptEnd,
        "C" => ShellMarkKind::OutputStart,
        "D" => {
            let code = match parts.next() {
                Some("") | None => None,
                Some(code) => Some(code.parse().ok()?),
            };
            ShellMarkKind::Finished(code)
        }
        _ => return None,
    };
    let token = parts
        .find_map(|part| part.strip_prefix("mosaicterm="))
        .map(str::to_owned);
    Some(ShellMark { kind, token })
}

fn parse_directory(body: &str) -> Option<PathBuf> {
    let (host, path) = body.strip_prefix("file://")?.split_once('/')?;
    if !host.is_empty()
        && !host.eq_ignore_ascii_case("localhost")
        && !hostname::get().is_ok_and(|local| local.to_string_lossy().eq_ignore_ascii_case(host))
    {
        return None;
    }
    if path.contains(['#', '?'])
        || path
            .as_bytes()
            .split(|&byte| byte == b'%')
            .skip(1)
            .any(|escape| {
                !escape
                    .get(..2)
                    .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit))
            })
    {
        return None;
    }

    let mut bytes = vec![b'/'];
    bytes.extend(percent_decode_str(path));
    if bytes.contains(&0) {
        return None;
    }
    #[cfg(unix)]
    let directory = {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(std::ffi::OsString::from_vec(bytes))
    };
    #[cfg(not(unix))]
    let directory = {
        let path = std::str::from_utf8(&bytes).ok()?;
        #[cfg(windows)]
        let path = {
            let drive_path = path.strip_prefix('/')?;
            if !matches!(
                drive_path.as_bytes(),
                [b'A'..=b'Z' | b'a'..=b'z', b':', b'/' | b'\\', ..]
            ) {
                return None;
            }
            drive_path
        };
        PathBuf::from(path)
    };
    directory.is_absolute().then_some(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_records_preserve_byte_order_and_handle_both_terminators() {
        let mut parser = TerminalEventParser::default();
        assert!(parser.observe(b"\x1b]13").is_empty());
        assert!(parser.observe(b"3;D;42\x1b").is_empty());
        let events = parser.observe(b"\\output\x1b]133;A\x07");
        assert_eq!(
            events,
            vec![
                (
                    1,
                    TerminalEvent::Shell(ShellMark {
                        kind: ShellMarkKind::Finished(Some(42)),
                        token: None
                    })
                ),
                (
                    15,
                    TerminalEvent::Shell(ShellMark {
                        kind: ShellMarkKind::Prompt,
                        token: None
                    })
                ),
            ]
        );
    }

    #[test]
    fn oversized_or_cancelled_osc_cannot_hide_the_next_event() {
        let mut parser = TerminalEventParser::default();
        let mut bytes = b"\x1b]7501;state=working:".to_vec();
        bytes.extend(vec![b'x'; MAX_OSC_BYTES]);
        bytes.extend(b"\x07\x1b]133;D;1\x18\x1b]7501;?\x07");
        assert_eq!(
            parser.observe(&bytes),
            vec![(bytes.len(), TerminalEvent::Program(ProgramEvent::Query))]
        );
    }

    #[cfg(unix)]
    #[test]
    fn osc_seven_decodes_paths_and_rejects_nonlocal_urls() {
        assert_eq!(
            parse_directory("file:///tmp/a%20b%23c%3B%E2%98%83"),
            Some(PathBuf::from("/tmp/a b#c;☃"))
        );
        assert!(parse_directory("file://192.0.2.1/tmp").is_none());
        assert!(parse_directory("file:///tmp/a%00b").is_none());
        assert!(parse_directory("file:///tmp/a%ZZ").is_none());
    }
}
