//! Bounded per-terminal program activity records from OSC 7501.

use base64::{engine::general_purpose, Engine as _};
use std::collections::VecDeque;

const MAX_RECORDS: usize = 64;
pub const FEATURE_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramState {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStatus {
    id: String,
    state: ProgramState,
    kind: Option<String>,
    progress: Option<u8>,
    app: Option<String>,
    title: Option<String>,
    message: Option<String>,
}

impl ProgramStatus {
    fn temporary(&self) -> bool {
        matches!(self.state, ProgramState::Working | ProgramState::Blocked)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProgramEvent {
    Query,
    Clear(String),
    Report(ProgramStatus),
}

impl ProgramEvent {
    pub(super) fn parse(body: &str) -> Option<Self> {
        if body == "?" {
            return Some(Self::Query);
        }
        let mut state = None;
        let mut id = "";
        let mut kind = None;
        let mut progress = None;
        let mut app = None;
        let mut title = None;
        let mut message = None;
        for pair in body.split(':') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            if key.is_empty()
                || !key.bytes().all(|byte| byte.is_ascii_lowercase())
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_.,+/=-".contains(&byte))
            {
                continue;
            }
            if key.len() > 16 {
                return None;
            }
            match key {
                "state" => state = Some(value),
                "id" => {
                    if value.len() > 128
                        || value.split('/').any(|segment| !valid_segment(segment))
                        || value.split('/').count() > 8
                    {
                        return None;
                    }
                    id = value;
                }
                "kind" => kind = Some(value),
                "progress" => progress = value.parse::<u8>().ok().filter(|value| *value <= 100),
                "app" => {
                    if value.len() > 32 {
                        return None;
                    }
                    app = valid_segment(value).then_some(value);
                }
                "title" => title = Some(decode_text(value, 256, 192)?),
                "msg" => message = Some(decode_text(value, 2732, 2048)?),
                _ => {}
            }
        }
        let state = match state? {
            "idle" => ProgramState::Idle,
            "working" => ProgramState::Working,
            "done" => ProgramState::Done,
            "blocked" => ProgramState::Blocked,
            "error" => ProgramState::Error,
            "clear" => return Some(Self::Clear(id.to_owned())),
            _ => return None,
        };

        Some(Self::Report(ProgramStatus {
            id: id.to_owned(),
            state,
            kind: kind
                .filter(|kind| {
                    state == ProgramState::Blocked
                        && matches!(*kind, "permission" | "question" | "auth")
                })
                .map(str::to_owned),
            progress: progress
                .filter(|_| matches!(state, ProgramState::Working | ProgramState::Blocked)),
            app: app.map(str::to_owned),
            title,
            message,
        }))
    }
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.+-".contains(&byte))
}

fn decode_text(encoded: &str, encoded_limit: usize, decoded_limit: usize) -> Option<String> {
    if encoded.len() > encoded_limit {
        return None;
    }
    let bytes = general_purpose::STANDARD
        .decode(encoded)
        .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(encoded))
        .ok()?;
    let mut text = String::from_utf8(bytes).ok()?;
    if text.len() > decoded_limit || text.chars().any(char::is_control) {
        return None;
    }
    text.retain(|ch| !matches!(ch, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}'));
    Some(text)
}

#[derive(Default)]
pub struct ProgramStatuses {
    records: VecDeque<ProgramStatus>,
}

impl ProgramStatuses {
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// A new shell prompt expires in-progress reports. Results remain visible.
    pub fn prompt_started(&mut self) {
        self.records.retain(|record| !record.temporary());
    }

    pub fn apply(&mut self, event: ProgramEvent) {
        match event {
            ProgramEvent::Query => {}
            ProgramEvent::Clear(id) if id.is_empty() => self.clear(),
            ProgramEvent::Clear(id) => {
                self.records.retain(|record| {
                    !record
                        .id
                        .strip_prefix(&id)
                        .is_some_and(|tail| tail.is_empty() || tail.starts_with('/'))
                });
            }
            ProgramEvent::Report(record) => {
                self.records.retain(|previous| previous.id != record.id);
                if self.records.len() == MAX_RECORDS {
                    self.records.pop_front();
                }
                self.records.push_back(record);
            }
        }
    }

    /// Return the most relevant report and its optional message.
    pub fn display(&self) -> Option<(String, Option<&str>)> {
        let record = self
            .records
            .iter()
            .rev()
            .min_by_key(|record| match record.state {
                ProgramState::Blocked => 0,
                ProgramState::Working => 1,
                ProgramState::Error | ProgramState::Done => 2,
                ProgramState::Idle => 3,
            })?;
        let state = match record.state {
            ProgramState::Idle => "Idle",
            ProgramState::Working => "Working",
            ProgramState::Done => "Done",
            ProgramState::Blocked => match record.kind.as_deref() {
                Some("permission") => "Waiting for approval",
                Some("auth") => "Waiting for authentication",
                _ => "Waiting for input",
            },
            ProgramState::Error => "Failed",
        };
        let name = record
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .or_else(|| self.inherited_app(record))
            .or_else(|| (!record.id.is_empty()).then_some(record.id.as_str()));
        let mut label = name.map_or_else(|| state.to_owned(), |name| format!("{name} · {state}"));
        if let Some(progress) = record.progress {
            label.push_str(&format!(" {progress}%"));
        }
        Some((label, record.message.as_deref()))
    }

    fn inherited_app<'a>(&'a self, record: &'a ProgramStatus) -> Option<&'a str> {
        if record.app.is_some() {
            return record.app.as_deref();
        }
        let mut id = record.id.as_str();
        while !id.is_empty() {
            id = id.rsplit_once('/').map_or("", |(parent, _)| parent);
            if let Some(app) = self
                .records
                .iter()
                .find(|parent| parent.id == id)
                .and_then(|parent| parent.app.as_deref())
            {
                return Some(app);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_inherit_app_expire_and_clear_subtrees() {
        let mut status = ProgramStatuses::default();
        for report in [
            "state=done:app=cargo",
            "state=blocked:id=build/test:kind=question:progress=25:msg=V2hhdD8",
            "state=working:id=other",
        ] {
            status.apply(ProgramEvent::parse(report).unwrap());
        }
        assert_eq!(
            status.display(),
            Some(("cargo · Waiting for input 25%".into(), Some("What?")))
        );
        status.apply(ProgramEvent::parse("state=clear:id=build").unwrap());
        assert_eq!(status.display().unwrap().0, "cargo · Working");
        status.prompt_started();
        assert_eq!(status.display().unwrap().0, "cargo · Done");
    }

    #[test]
    fn rejects_invalid_ids_encoded_text_and_controls() {
        for report in [
            "state=working:id=build//test",
            "state=working:id=",
            "state=working:msg=bmV3CmxpbmU=",
            "state=working:msg=denied_",
            "state=unsupported",
        ] {
            assert_eq!(ProgramEvent::parse(report), None, "{report}");
        }
        assert!(ProgramEvent::parse("state=working:!bad=pair:unused=ignored").is_some());
    }

    #[test]
    fn records_are_bounded_and_oldest_updates_are_evicted() {
        let mut status = ProgramStatuses::default();
        for i in 0..=MAX_RECORDS {
            status.apply(ProgramEvent::parse(&format!("state=working:id=job{i}")).unwrap());
        }
        assert_eq!(status.records.len(), MAX_RECORDS);
        assert_eq!(status.records.front().unwrap().id, "job1");
    }
}
