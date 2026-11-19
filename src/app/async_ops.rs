//! Background terminal lifecycle operations.

use mosaicterm::config::TerminalConfig;
use mosaicterm::error::Result;
use mosaicterm::models::{ShellType, TerminalSession};
use mosaicterm::pty::PtyManager;
use mosaicterm::session::persistent::SessionSnapshot;
use mosaicterm::terminal::TerminalFactory;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{AsyncRequest, AsyncResult, ShellLaunch, StartedShell};

pub async fn async_operation_loop(
    request_rx: &mut mpsc::UnboundedReceiver<AsyncRequest>,
    result_tx: mpsc::UnboundedSender<AsyncResult>,
    pty_manager: Arc<PtyManager>,
    terminal_factory: TerminalFactory,
) {
    while let Some(request) = request_rx.recv().await {
        match request {
            AsyncRequest::StartShell {
                pane_id,
                config,
                working_dir,
                launch,
            } => {
                let result = start_shell(
                    &pty_manager,
                    &terminal_factory,
                    &config,
                    working_dir,
                    launch,
                )
                .await;
                if let Err(error) = result_tx.send(AsyncResult::ShellStarted { pane_id, result }) {
                    if let AsyncResult::ShellStarted {
                        result: Ok(started),
                        ..
                    } = error.0
                    {
                        started.terminal.close().await;
                    }
                }
            }
            AsyncRequest::ListSessions(host) => {
                let result = tokio::task::spawn_blocking(move || host.list())
                    .await
                    .map_err(std::io::Error::other)
                    .map_err(mosaicterm::error::Error::from)
                    .and_then(|result| result);
                let _ = result_tx.send(AsyncResult::SessionsListed(result));
            }
            AsyncRequest::EndSession(session) => {
                let target = session.clone();
                let result = tokio::task::spawn_blocking(move || target.end())
                    .await
                    .map_err(std::io::Error::other)
                    .map_err(mosaicterm::error::Error::from)
                    .and_then(|result| result);
                let _ = result_tx.send(AsyncResult::SessionEnded { session, result });
            }
            AsyncRequest::CloseTerminal(terminal) => terminal.close().await,
            AsyncRequest::SendInterrupt { pane_id, handle } => {
                let result = pty_manager.send_input(&handle, &[3]).await;
                let _ = result_tx.send(AsyncResult::InterruptSent { pane_id, result });
            }
        }
    }
}

async fn start_shell(
    pty_manager: &PtyManager,
    terminal_factory: &TerminalFactory,
    config: &TerminalConfig,
    working_dir: PathBuf,
    launch: ShellLaunch,
) -> Result<StartedShell> {
    let (executable, shell_type) = config.shell();
    if let ShellLaunch::Persistent { session, create } = launch {
        let remote = session.host.ssh.is_some();
        let shell_type = if remote { ShellType::Bash } else { shell_type };
        if create {
            let mut args = if remote {
                Vec::new()
            } else {
                config.shell_args.clone()
            };
            let binary = if remote {
                "bash".to_owned()
            } else {
                executable.to_string_lossy().into_owned()
            };
            let shell = if shell_type == ShellType::Bash && (remote || args.is_empty()) {
                let rc = mosaicterm::pty::shell_state::persistent_bash_rc(&session.name);
                let bootstrap = r#"umask 077
rc=$(mktemp) || exit 1
printf '%s' "$1" > "$rc" || { rm -f -- "$rc"; exit 1; }
MOSAICTERM_BOOTSTRAP=$rc
export MOSAICTERM_BOOTSTRAP
exec "$2" --rcfile "$rc" --noediting"#;
                vec![
                    "sh".into(),
                    "-c".into(),
                    bootstrap.into(),
                    "mosaicterm".into(),
                    rc,
                    binary,
                ]
            } else {
                args.insert(0, binary);
                args
            };
            let target = session.clone();
            tokio::task::spawn_blocking(move || {
                target.create(&shell, (!remote).then_some(working_dir.as_path()))
            })
            .await
            .map_err(std::io::Error::other)??;
        }
        let (handle, mut snapshot) = pty_manager.attach_persistent(session, !create).await?;
        if snapshot.exit_code.is_none() {
            snapshot.idle = None;
        }
        return attach_terminal(terminal_factory, shell_type, handle, snapshot);
    }

    let mut environment: std::collections::HashMap<String, String> = std::env::vars().collect();
    environment.insert("TERM".into(), "xterm-256color".into());
    environment.insert("MOSAICTERM".into(), "1".into());

    let session = TerminalSession::with_environment(shell_type, working_dir, environment);
    let mut terminal = terminal_factory.create_with_shell(session, shell_type);
    terminal
        .initialize_configured_shell(&executable, config.shell_args.clone())
        .await?;
    Ok(StartedShell {
        terminal: Box::new(terminal),
        snapshot: None,
    })
}

fn attach_terminal(
    terminal_factory: &TerminalFactory,
    shell_type: ShellType,
    handle: mosaicterm::pty::PtyHandle,
    snapshot: SessionSnapshot,
) -> Result<StartedShell> {
    let session = TerminalSession::new(shell_type, snapshot.cwd.clone());
    let mut terminal = terminal_factory.create_with_shell(session, shell_type);
    terminal.initialize_with_pty(handle)?;
    Ok(StartedShell {
        terminal: Box::new(terminal),
        snapshot: Some(snapshot),
    })
}
