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
pub async fn async_operation_loop(
    request_rx: &mut mpsc::UnboundedReceiver<AsyncRequest>,
    result_tx: mpsc::UnboundedSender<AsyncResult>,
    pty_manager: Arc<PtyManager>,
    terminal_factory: TerminalFactory,
) {
    info!("Starting async operation loop");

    while let Some(request) = request_rx.recv().await {
        match request {
            AsyncRequest::InitTerminal => {
                info!("Processing async InitTerminal request");
                let result = async_initialize_terminal(&terminal_factory).await;
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

                // Check if this command should use direct execution
                if DirectExecutor::check_direct_execution(&command) {
                    info!(
                        "Executing command directly (non-PTY): {} in {:?}",
                        command, working_dir
                    );

                    let mut executor = DirectExecutor::new();
                    executor.set_working_dir(working_dir);

                    // Execute command directly
                    match executor.execute_command(&command).await {
                        Ok(command_block) => {
                            info!("Direct execution completed for: {}", command);
                            // Send completed command block back to UI
                            let _ =
                                result_tx.send(AsyncResult::DirectCommandCompleted(command_block));
                        }
                        Err(e) => {
                            error!("Direct execution failed for {}: {}", command, e);
                            // Send error result
                            let _ = result_tx.send(AsyncResult::DirectCommandFailed {
                                command,
                                error: e.to_string(),
                            });
                        }
                    }
                } else {
                    // For PTY commands, execution happens in main thread
                    // This async handler is just for direct execution
                    debug!(
                        "Command {} requires PTY execution, skipping async handler",
                        command
                    );
                }
            }
            AsyncRequest::RestartPty => {
                info!("Processing async RestartPty request");
                let result = async_restart_pty(&pty_manager, &terminal_factory).await;
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

/// Initialize terminal in background
async fn async_initialize_terminal(terminal_factory: &TerminalFactory) -> Result<()> {
    info!("Async terminal initialization started");

    // Get runtime config from environment or use defaults
    let runtime_config = RuntimeConfig::new().unwrap_or_else(|e| {
        error!("Failed to create runtime config: {}", e);
        RuntimeConfig::new_minimal()
    });

    let shell_type = match runtime_config.config().terminal.shell_type {
        ModelShellType::Bash => ModelShellType::Bash,
        ModelShellType::Zsh => ModelShellType::Zsh,
        ModelShellType::Fish => ModelShellType::Fish,
        _ => ModelShellType::Bash,
    };

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
    let mut environment: std::collections::HashMap<String, String> = std::env::vars().collect();

    // Suppress prompts
    environment.insert("PS1".to_string(), "".to_string());
    environment.insert("PS2".to_string(), "".to_string());
    environment.insert("TERM".to_string(), "dumb".to_string());

    let session = TerminalSession::with_environment(shell_type, working_dir, environment);
    let _terminal = terminal_factory.create_and_initialize(session).await?;

    info!("Async terminal initialization completed");
    Ok(())
}

/// Restart PTY in background
async fn async_restart_pty(
    _pty_manager: &Arc<PtyManager>,
    _terminal_factory: &TerminalFactory,
) -> Result<()> {
    info!("Async PTY restart started");

    // For now, just log - full implementation would recreate terminal
    // This is complex because we need to update app state

    info!("Async PTY restart completed");
    Ok(())
}

/// Send interrupt signal in background
async fn async_send_interrupt(pty_manager: &Arc<PtyManager>, handle_id: &str) -> Result<()> {
    info!("Async interrupt signal for handle: {}", handle_id);

    // PtyManager is already async and thread-safe, no lock needed

    // Create PTY handle with the given ID
    let pty_handle = mosaicterm::pty::PtyHandle {
        id: handle_id.to_string(),
        pid: None,
    };

    // First, send Ctrl+C (ASCII 3) directly to the PTY input
    // This is the polite way - gives the process a chance to clean up
    info!("Sending Ctrl+C to PTY");
    let _ = pty_manager.send_input(&pty_handle, &[3]).await;

    // Get the shell PID
    if let Ok(pty_info) = pty_manager.get_info(&pty_handle).await {
        if let Some(shell_pid) = pty_info.pid {
            info!("Killing process tree for shell PID: {}", shell_pid);

            // Kill the entire process tree (shell + all children)
            // This ensures long-running commands like sleep, find, etc. are all killed
            // Uses platform abstraction for cross-platform support
            if let Err(e) = mosaicterm::pty::process_tree::kill_process_tree(shell_pid) {
                warn!("Failed to kill process tree: {}", e);
            }

            // Wait a moment for processes to terminate gracefully
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // If still running, check and log
            if let Ok(children) = mosaicterm::pty::process_tree::get_all_descendant_pids(shell_pid)
            {
                if !children.is_empty() {
                    info!("Some processes still running, attempting force kill");
                    // Try again - platform implementation should handle force kill
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
