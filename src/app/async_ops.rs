//! Async Operations
//!
//! Background task processing for terminal initialization, command execution,
//! and PTY management. This module runs in a separate tokio runtime to avoid
//! blocking the UI thread.
//!
//! ## Architecture
//!
//! The async operation system uses channels to communicate between the UI thread
//! and background tasks:
//!
//! ```text
//! ┌──────────────────┐          ┌──────────────────┐
//! │    UI Thread     │          │  Background Task │
//! │  (MosaicTermApp) │          │ (async_ops loop) │
//! │                  │          │                  │
//! │  async_tx ─────────────────▶│  request_rx      │
//! │                  │          │                  │
//! │  async_rx ◀─────────────────│  result_tx       │
//! └──────────────────┘          └──────────────────┘
//! ```
//!
//! ## Supported Operations
//!
//! - **InitTerminal**: Initialize the terminal and PTY in the background
//! - **ExecuteCommand**: Execute commands directly (for non-PTY commands)
//! - **RestartPty**: Restart the PTY session after errors or interactive programs
//! - **SendInterrupt**: Send Ctrl+C and kill signals to running processes
//!
//! ## Usage
//!
//! The async loop is spawned once at app startup and runs for the lifetime of
//! the application. The UI thread sends requests via `async_tx` and polls for
//! results via `async_rx`.

use mosaicterm::config::RuntimeConfig;
use mosaicterm::error::Result;
use mosaicterm::execution::DirectExecutor;
use mosaicterm::models::{ShellType as ModelShellType, TerminalSession};
use mosaicterm::pty::PtyManager;
use mosaicterm::terminal::TerminalFactory;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::{AsyncRequest, AsyncResult};

/// Run the async operation processing loop
///
/// This function runs in a background task and processes requests from the UI thread.
/// It handles terminal initialization, command execution, and PTY lifecycle events.
///
/// `runtime_config` is the same config the UI loaded so the async side uses
/// identical shell type, paths, and theme settings.
pub async fn async_operation_loop(
    request_rx: &mut mpsc::UnboundedReceiver<AsyncRequest>,
    result_tx: mpsc::UnboundedSender<AsyncResult>,
    pty_manager: Arc<PtyManager>,
    terminal_factory: TerminalFactory,
    runtime_config: RuntimeConfig,
) {
    info!("Starting async operation loop");

    while let Some(request) = request_rx.recv().await {
        match request {
            AsyncRequest::InitTerminal => {
                info!("Processing async InitTerminal request");
                let result = async_initialize_terminal(&terminal_factory, &runtime_config).await;
                match result {
                    Ok(()) => {
                        let _ = result_tx.send(AsyncResult::TerminalInitialized);
                    }
                    Err(e) => {
                        let _ = result_tx.send(AsyncResult::TerminalInitFailed(e.to_string()));
                    }
                }
            }
            AsyncRequest::ExecuteCommand(command, working_dir) => {
                info!(
                    "Processing async ExecuteCommand: {} in {:?}",
                    command, working_dir
                );

                if DirectExecutor::check_direct_execution(&command) {
                    info!(
                        "Executing command directly (non-PTY): {} in {:?}",
                        command, working_dir
                    );

                    let mut executor = DirectExecutor::new();
                    executor.set_working_dir(working_dir);

                    match executor.execute_command(&command).await {
                        Ok(command_block) => {
                            info!("Direct execution completed for: {}", command);
                            let _ =
                                result_tx.send(AsyncResult::DirectCommandCompleted(command_block));
                        }
                        Err(e) => {
                            error!("Direct execution failed for {}: {}", command, e);
                            let _ = result_tx.send(AsyncResult::DirectCommandFailed {
                                command,
                                error: e.to_string(),
                            });
                        }
                    }
                } else {
                    debug!(
                        "Command {} requires PTY execution, skipping async handler",
                        command
                    );
                }
            }
            AsyncRequest::RestartPty => {
                info!("Processing async RestartPty request");
                let result =
                    async_restart_pty(&pty_manager, &terminal_factory, &runtime_config).await;
                match result {
                    Ok(()) => {
                        let _ = result_tx.send(AsyncResult::PtyRestarted);
                    }
                    Err(e) => {
                        let _ = result_tx.send(AsyncResult::PtyRestartFailed(e.to_string()));
                    }
                }
            }
            AsyncRequest::SendInterrupt(handle_id) => {
                info!("Processing async SendInterrupt for handle: {}", handle_id);
                let result = async_send_interrupt(&pty_manager, &handle_id).await;
                match result {
                    Ok(()) => {
                        let _ = result_tx.send(AsyncResult::InterruptSent);
                    }
                    Err(e) => {
                        let _ = result_tx.send(AsyncResult::InterruptFailed(e.to_string()));
                    }
                }
            }
        }
    }

    info!("Async operation loop ended");
}

/// Resolve a sensible starting working directory.
///
/// Prefers the user's home directory over `current_dir()` because when
/// the binary is launched from a desktop shortcut or file manager,
/// `current_dir()` is often `/` or another unintuitive location.
fn resolve_working_dir(config: &RuntimeConfig) -> std::path::PathBuf {
    if let Some(dir) = &config.config().terminal.working_directory {
        if dir.is_dir() {
            return dir.clone();
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if cwd != std::path::Path::new("/") {
            return cwd;
        }
    }

    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"))
}

/// Initialize terminal in background using the app's runtime configuration.
async fn async_initialize_terminal(
    terminal_factory: &TerminalFactory,
    runtime_config: &RuntimeConfig,
) -> Result<()> {
    info!("Async terminal initialization started");

    let shell_type = match runtime_config.config().terminal.shell_type {
        ModelShellType::Bash => ModelShellType::Bash,
        ModelShellType::Zsh => ModelShellType::Zsh,
        ModelShellType::Fish => ModelShellType::Fish,
        _ => ModelShellType::Bash,
    };

    let working_dir = resolve_working_dir(runtime_config);
    let mut environment: std::collections::HashMap<String, String> = std::env::vars().collect();

    environment.insert("PS1".to_string(), "".to_string());
    environment.insert("PS2".to_string(), "".to_string());
    // Use xterm-256color so tools/TUIs work, matching the main init path
    environment.insert("TERM".to_string(), "xterm-256color".to_string());

    let session = TerminalSession::with_environment(shell_type, working_dir, environment);
    let _terminal = terminal_factory.create_and_initialize(session).await?;

    info!("Async terminal initialization completed");
    Ok(())
}

/// Restart PTY in background using the app's runtime configuration.
async fn async_restart_pty(
    _pty_manager: &Arc<PtyManager>,
    terminal_factory: &TerminalFactory,
    runtime_config: &RuntimeConfig,
) -> Result<()> {
    info!("Async PTY restart started");

    let shell_type = match runtime_config.config().terminal.shell_type {
        ModelShellType::Bash => ModelShellType::Bash,
        ModelShellType::Zsh => ModelShellType::Zsh,
        ModelShellType::Fish => ModelShellType::Fish,
        _ => ModelShellType::Bash,
    };

    let working_dir = resolve_working_dir(runtime_config);
    let environment: std::collections::HashMap<String, String> = std::env::vars().collect();
    let session = TerminalSession::with_environment(shell_type, working_dir, environment);

    let _terminal = terminal_factory.create_and_initialize(session).await?;

    info!("Async PTY restart completed");
    Ok(())
}

/// Send interrupt signal in background
async fn async_send_interrupt(pty_manager: &Arc<PtyManager>, handle_id: &str) -> Result<()> {
    info!("Async interrupt signal for handle: {}", handle_id);

    let pty_handle = mosaicterm::pty::PtyHandle {
        id: handle_id.to_string(),
        pid: None,
    };

    info!("Sending Ctrl+C to PTY");
    let _ = pty_manager.send_input(&pty_handle, &[3]).await;

    if let Ok(pty_info) = pty_manager.get_info(&pty_handle).await {
        if let Some(shell_pid) = pty_info.pid {
            info!("Killing process tree for shell PID: {}", shell_pid);

            if let Err(e) = mosaicterm::pty::process_tree::kill_process_tree(shell_pid) {
                warn!("Failed to kill process tree: {}", e);
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            if let Ok(children) = mosaicterm::pty::process_tree::get_all_descendant_pids(shell_pid)
            {
                if !children.is_empty() {
                    info!("Some processes still running, attempting force kill");
                    let _ = mosaicterm::pty::process_tree::kill_process_tree(shell_pid);
                }
            }
        } else {
            return Err(mosaicterm::error::Error::NoPidAvailable {
                handle_id: handle_id.to_string(),
            });
        }
    } else {
        return Err(mosaicterm::error::Error::PtyHandleNotFound {
            handle_id: handle_id.to_string(),
        });
    }

    Ok(())
}
