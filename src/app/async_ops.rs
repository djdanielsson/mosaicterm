//! Background terminal lifecycle operations.

use mosaicterm::config::TerminalConfig;
use mosaicterm::error::Result;
use mosaicterm::models::TerminalSession;
use mosaicterm::pty::PtyManager;
use mosaicterm::terminal::{Terminal, TerminalFactory};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{AsyncRequest, AsyncResult};

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
            } => {
                let result = start_shell(&terminal_factory, &config, working_dir).await;
                if let Err(error) = result_tx.send(AsyncResult::ShellStarted { pane_id, result }) {
                    if let AsyncResult::ShellStarted {
                        result: Ok(terminal),
                        ..
                    } = error.0
                    {
                        terminal.close().await;
                    }
                }
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
    terminal_factory: &TerminalFactory,
    config: &TerminalConfig,
    working_dir: PathBuf,
) -> Result<Box<Terminal>> {
    let (executable, shell_type) = config.shell();
    let mut environment: std::collections::HashMap<String, String> = std::env::vars().collect();
    environment.insert("TERM".into(), "xterm-256color".into());
    environment.insert("MOSAICTERM".into(), "1".into());

    let session = TerminalSession::with_environment(shell_type, working_dir, environment);
    let mut terminal = terminal_factory.create_with_shell(session, shell_type);
    terminal
        .initialize_configured_shell(&executable, config.shell_args.clone())
        .await?;
    Ok(Box::new(terminal))
}
