use super::pane_tree::{SavedLayout, SplitAxis};
use super::{AsyncRequest, MosaicTermApp, PaneSession, ShellLaunch};
use eframe::egui;
use mosaicterm::error::Result;
use mosaicterm::models::{CommandBlock, OutputLine};
use mosaicterm::session::persistent::{PersistentSession, SessionHost, SessionSnapshot};
use std::io::Write;
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct SessionPanel {
    pub open: bool,
    host: String,
    listed: Vec<PersistentSession>,
    saved: Option<SavedLayout>,
    message: Option<String>,
    listing: bool,
    pub(super) layout_path: Option<PathBuf>,
}

impl MosaicTermApp {
    pub(super) fn configured_host(&self) -> SessionHost {
        SessionHost {
            ssh: self
                .runtime_config
                .config()
                .session
                .ssh_host
                .clone()
                .filter(|host| !host.is_empty()),
        }
    }

    pub(super) fn is_persistent_remote(&self) -> bool {
        self.pane
            .persistent
            .as_ref()
            .is_some_and(|session| session.host.ssh.is_some())
    }

    pub(super) fn initialize_session_layout(&mut self) {
        let working_dir = Self::initial_working_dir(&self.runtime_config.config().terminal);
        let config = &self.runtime_config.config().session;
        if !config.persistence {
            self.request_shell(working_dir);
            return;
        }

        if self.sessions.layout_path.is_none() {
            self.sessions.layout_path = dirs::state_dir()
                .or_else(dirs::config_dir)
                .map(|dir| dir.join("mosaicterm").join("pane-layout.json"));
        }
        if let Some(path) = &self.sessions.layout_path {
            match std::fs::read(path) {
                Ok(data) if data.len() <= 128 * 1024 => {
                    self.sessions.saved = serde_json::from_slice(&data).ok();
                }
                Ok(_) => tracing::warn!("Ignoring oversized session layout"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => tracing::warn!("Could not read session layout: {error}"),
            }
        }

        if config.auto_restore && self.sessions.saved.is_some() {
            self.restore_saved_layout();
        } else if self.sessions.saved.is_some() || self.configured_host().ssh.is_some() {
            self.open_sessions();
        } else {
            self.request_shell(working_dir);
        }
    }

    pub(super) fn save_session_layout(&mut self) {
        if let Some(layout) = self.pane_tree.saved_layout(self.pane.persistent.as_ref()) {
            self.sessions.saved = Some(layout);
            self.write_saved_layout();
        }
    }

    fn write_saved_layout(&self) {
        let Some(path) = &self.sessions.layout_path else {
            return;
        };
        let result = (|| -> anyhow::Result<()> {
            if let Some(layout) = &self.sessions.saved {
                let dir = path.parent().expect("session layout directory");
                std::fs::create_dir_all(dir)?;
                let mut file = tempfile::NamedTempFile::new_in(dir)?;
                serde_json::to_writer(&mut file, layout)?;
                file.flush()?;
                file.persist(path)?;
            } else if let Err(error) = std::fs::remove_file(path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    return Err(error.into());
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            tracing::warn!("Could not save pane layout: {error}");
        }
    }

    fn restore_saved_layout(&mut self) {
        let Some(saved) = self.sessions.saved.take() else {
            return;
        };
        if self.pane.terminal.is_some()
            || self.pane.initializing
            || self.pane_tree.pane_count() != 1
        {
            self.sessions.saved = Some(saved);
            return;
        }
        let working_dir = Self::initial_working_dir(&self.runtime_config.config().terminal);
        let config = &self.runtime_config.config().terminal;
        let Some(tree) = saved.restore(&mut self.state_manager, &working_dir, config.shell().1)
        else {
            self.sessions.message = Some("Invalid saved pane layout".into());
            self.open_sessions();
            return;
        };
        self.pane_tree = tree;
        self.pane = PaneSession::default();
        self.switch_pane_session("");
        let active = self.pane_tree.active_id().to_owned();
        for id in self.pane_tree.all_pane_ids() {
            self.activate_pane(&id);
            if self.pane.persistent.is_some() {
                self.request_shell(working_dir.clone());
            }
        }
        self.activate_pane(&active);
        self.sessions.open = false;
    }

    pub(super) fn open_sessions(&mut self) {
        if !self.sessions.open {
            self.sessions.host = self
                .pane
                .persistent
                .as_ref()
                .and_then(|session| session.host.ssh.clone())
                .or_else(|| self.configured_host().ssh)
                .unwrap_or_default();
            self.sessions.open = true;
            self.refresh_sessions();
        }
    }

    fn listed_host(&self) -> SessionHost {
        let host = self.sessions.host.trim();
        SessionHost {
            ssh: (!host.is_empty()).then(|| host.to_owned()),
        }
    }

    fn refresh_sessions(&mut self) {
        self.sessions.listed.clear();
        self.sessions.message = None;
        self.sessions.listing = self
            .async_tx
            .send(AsyncRequest::ListSessions(self.listed_host()))
            .is_ok();
    }

    pub(super) fn finish_session_listing(&mut self, result: Result<Vec<PersistentSession>>) {
        self.sessions.listing = false;
        match result {
            Ok(listed) => self.sessions.listed = listed,
            Err(error) => self.sessions.message = Some(error.to_string()),
        }
    }

    pub(super) fn render_sessions(&mut self, ctx: &egui::Context) {
        let mut open = self.sessions.open;
        egui::Window::new("Sessions")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("SSH host");
                    ui.add(egui::TextEdit::singleline(&mut self.sessions.host).hint_text("Local"));
                    if ui
                        .add_enabled(!self.sessions.listing, egui::Button::new("List sessions"))
                        .clicked()
                    {
                        self.refresh_sessions();
                    }
                });
                if let Some(message) = &self.sessions.message {
                    ui.label(message);
                }
                if self.sessions.listing {
                    ui.spinner();
                }

                let can_restore = self.sessions.saved.is_some()
                    && self.pane.terminal.is_none()
                    && !self.pane.initializing
                    && self.pane_tree.pane_count() == 1
                    && self.pane.persistent.is_none();
                if can_restore && ui.button("Restore previous layout").clicked() {
                    self.restore_saved_layout();
                }

                for session in self.sessions.listed.clone() {
                    ui.horizontal(|ui| {
                        ui.label(&session.name);
                        if ui.button("Attach").clicked() {
                            self.open_persistent(session.clone(), false);
                        }
                        if ui.button("End session").clicked() {
                            self.end_session(session.clone());
                        }
                    });
                }
                if self.pane.terminal.is_none() && !self.pane.initializing {
                    if let Some(session) = self.pane.persistent.clone() {
                        if ui.button("Reconnect to this pane").clicked() {
                            self.open_persistent(session, false);
                        }
                    }
                }
                if ui.button("New persistent shell").clicked() {
                    self.open_persistent(PersistentSession::new(self.listed_host()), true);
                }
            });
        self.sessions.open &= open;
    }

    fn pane_for_session(&self, session: &PersistentSession) -> Option<String> {
        self.pane_tree.all_pane_ids().into_iter().find(|id| {
            let saved = if id == self.pane_tree.active_id() {
                self.pane.persistent.as_ref()
            } else {
                self.pane_tree
                    .find_pane(id)
                    .and_then(|pane| pane.session.persistent.as_ref())
            };
            saved == Some(session)
        })
    }

    fn open_persistent(&mut self, session: PersistentSession, create: bool) {
        let working_dir = Self::initial_working_dir(&self.runtime_config.config().terminal);
        if let Some(pane_id) = self.pane_for_session(&session) {
            self.activate_pane(&pane_id);
        } else if self.pane.terminal.is_some() || self.pane.initializing {
            self.add_split(SplitAxis::Horizontal, working_dir.clone());
        }
        if self.pane.terminal.is_none() && !self.pane.initializing {
            self.launch_shell(working_dir, ShellLaunch::Persistent { session, create });
        }
        self.sessions.open = false;
    }

    fn end_session(&mut self, session: PersistentSession) {
        let _ = self.async_tx.send(AsyncRequest::EndSession(session));
    }

    pub(super) fn finish_session_end(&mut self, ended: &PersistentSession, result: Result<()>) {
        if let Err(error) = result {
            self.sessions.message = Some(error.to_string());
            return;
        }
        let pane_id = self.pane_for_session(ended).unwrap_or_default();
        let active = pane_id == self.pane_tree.active_id();
        if let Some(pane) = self.pane_tree.find_pane_mut(&pane_id) {
            let state = if active {
                &mut self.pane
            } else {
                &mut pane.session
            };
            if let Some(terminal) = state.terminal.take() {
                let _ = self.async_tx.send(AsyncRequest::CloseTerminal(terminal));
            }
            state.persistent = None;
            state.program_statuses.clear();
            state.status_message = Some("Session ended".into());
            if let Some(session) = self.state_manager.get_session_mut(&pane.session_id) {
                session.pty_handle_id = None;
                session.last_command_time = None;
                if let Some(id) = state.pending_block.take() {
                    if let Some(block) = session
                        .command_history
                        .iter_mut()
                        .rev()
                        .find(|block| block.id == id)
                    {
                        block.status = mosaicterm::models::ExecutionStatus::Cancelled;
                    }
                }
            }
            self.sessions.saved = self.pane_tree.saved_layout(self.pane.persistent.as_ref());
            self.write_saved_layout();
        } else if let Some(mut layout) = self.sessions.saved.take() {
            self.sessions.saved = layout.forget(ended).then_some(layout);
            self.write_saved_layout();
        }
        self.refresh_sessions();
    }

    pub(super) fn apply_session_snapshot(&mut self, pane_id: &str, snapshot: SessionSnapshot) {
        let active = pane_id == self.pane_tree.active_id();
        let Some(pane) = self.pane_tree.find_pane_mut(pane_id) else {
            return;
        };
        let state = if active {
            &mut self.pane
        } else {
            &mut pane.session
        };
        state.shell_markers_ready = snapshot.idle.is_some();
        if let Some(host) = state
            .persistent
            .as_ref()
            .and_then(|session| session.host.ssh.as_deref())
        {
            state.status_message = Some(format!("Attached to {host}"));
        }
        let Some(session) = self.state_manager.get_session_mut(&pane.session_id) else {
            return;
        };
        if snapshot.alternate_screen {
            if let Some(handle) = state
                .terminal
                .as_ref()
                .and_then(|terminal| terminal.pty_handle())
            {
                state
                    .tui_overlay
                    .start("Reattached terminal".into(), handle.id.clone());
                state.tui_overlay.note_alt_screen_enter();
                let screen = snapshot.recent.replace('\n', "\r\n");
                state.tui_overlay.add_raw_output(screen.as_bytes());
            }
        }
        if !snapshot.recent.is_empty() {
            let mut block =
                CommandBlock::new("Recent terminal output".into(), snapshot.cwd.clone());
            block.status = mosaicterm::models::ExecutionStatus::Completed;
            block.add_output_lines(snapshot.recent.lines().map(OutputLine::new).collect());
            session.command_history.push(block);
        }
        if snapshot.idle == Some(false) && state.pending_block.is_none() {
            let mut block = CommandBlock::new("Reattached running command".into(), snapshot.cwd);
            block.mark_running();
            state.pending_block = Some(block.id.clone());
            session.command_history.push(block);
        } else if snapshot.idle == Some(true) {
            if let Some(id) = state.pending_block.take() {
                if let Some(block) = session
                    .command_history
                    .iter_mut()
                    .rev()
                    .find(|block| block.id == id)
                {
                    if let Some(code) = snapshot.exit_code {
                        let elapsed = session
                            .last_command_time
                            .map(|time| time.elapsed())
                            .unwrap_or_default();
                        block.mark_completed_with_code(elapsed, code);
                    }
                }
                session.last_command_time = None;
            }
        }
    }
}
