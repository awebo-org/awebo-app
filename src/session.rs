//! Session history tracking.
//!
//! Records every command executed in each tab so sessions can be reviewed
//! later from the side panel.  Each tab is assigned a `SessionId` on
//! creation.  When a command block finishes, its data is copied into the
//! corresponding `Session`.

use std::time::{Duration, Instant, SystemTime};

use crate::blocks::{BlockList, CommandBlock, StyledLine};
use crate::prompt::{PromptInfo, PromptSegment, SegmentKind};

/// Opaque session identifier (monotonically increasing).
pub type SessionId = u64;

/// A single recorded command within a session.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionEntry {
    pub command: String,
    pub output: Vec<StyledLine>,
    pub cwd: String,
    pub duration: Option<Duration>,
    pub exit_code: Option<i32>,
    pub is_error: bool,
    pub started_at: SystemTime,
}

/// A complete session — one per tab lifetime.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub title: String,
    pub entries: Vec<SessionEntry>,
    pub created_at: SystemTime,
}

impl Session {
    fn new(id: SessionId) -> Self {
        Self {
            id,
            title: String::new(),
            entries: Vec::new(),
            created_at: SystemTime::now(),
        }
    }

    /// Derive a display title from the first command or CWD.
    pub fn display_title(&self) -> String {
        if !self.title.is_empty() {
            return self.title.clone();
        }
        if let Some(first) = self.entries.first() {
            if !first.command.is_empty() {
                return first.command.clone();
            }
            if !first.cwd.is_empty() {
                return first.cwd.clone();
            }
        }
        format!("Session {}", self.id)
    }

    /// Reconstruct a BlockList from the recorded session entries.
    pub fn to_block_list(&self) -> BlockList {
        let mut bl = BlockList::new();
        for entry in &self.entries {
            let cwd_fg = (160, 162, 170);
            let prompt = PromptInfo {
                segments: vec![PromptSegment {
                    kind: SegmentKind::Cwd,
                    text: entry.cwd.clone(),
                    fg: cwd_fg,
                }],
                diff_additions: 0,
                diff_deletions: 0,
            };
            let mut block = CommandBlock {
                prompt,
                command: entry.command.clone(),
                output: entry.output.clone(),
                started: Instant::now(),
                duration: entry.duration,
                selected: false,
                thinking: false,
                is_error: entry.is_error,
                exit_code: entry.exit_code,
                pending_line: None,
                checkpoint_at_start: 0,
                restored: true,
                agent_step: None,
            };
            if block.duration.is_none() {
                block.duration = Some(Duration::ZERO);
            }
            bl.blocks.push(block);
        }
        bl.bump_generation();
        bl
    }
}

fn sessions_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("awebo")
        .join("sessions")
}

/// Manages all sessions (active + past) with JSON persistence.
pub struct SessionManager {
    sessions: Vec<Session>,
    next_id: SessionId,
    /// When true, skip all disk I/O (for tests).
    #[cfg(test)]
    in_memory: bool,
}

impl SessionManager {
    /// Load sessions from disk, or start empty.
    pub fn new() -> Self {
        let dir = sessions_dir();
        let mut sessions = Vec::new();
        let mut max_id: SessionId = 0;

        if dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    match std::fs::read_to_string(&path) {
                        Ok(data) => match serde_json::from_str::<Session>(&data) {
                            Ok(session) => {
                                if session.id > max_id {
                                    max_id = session.id;
                                }
                                sessions.push(session);
                            }
                            Err(e) => log::warn!("Failed to parse session {:?}: {}", path, e),
                        },
                        Err(e) => log::warn!("Failed to read session {:?}: {}", path, e),
                    }
                }
            }
        }

        sessions.sort_by_key(|s| s.id);

        Self {
            sessions,
            next_id: max_id + 1,
            #[cfg(test)]
            in_memory: false,
        }
    }

    /// Create a new session and return its ID.
    pub fn create_session(&mut self) -> SessionId {
        let id = self.next_id;
        self.next_id += 1;
        let session = Session::new(id);
        #[cfg(not(test))]
        save_session_to_disk(&session);
        #[cfg(test)]
        if !self.in_memory {
            save_session_to_disk(&session);
        }
        self.sessions.push(session);
        id
    }

    /// Record a finished command block into the given session.
    pub fn record_block(
        &mut self,
        session_id: SessionId,
        block: &CommandBlock,
        prompt: &PromptInfo,
    ) {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id) {
            let cwd = prompt
                .segments
                .iter()
                .find(|s| s.kind == crate::prompt::SegmentKind::Cwd)
                .map(|s| s.text.clone())
                .unwrap_or_default();

            let entry = SessionEntry {
                command: block.command.clone(),
                output: block.output.clone(),
                cwd,
                duration: block.duration,
                exit_code: block.exit_code,
                is_error: block.is_error,
                started_at: SystemTime::now()
                    .checked_sub(block.elapsed())
                    .unwrap_or(SystemTime::now()),
            };

            session.entries.push(entry);
            #[cfg(not(test))]
            save_session_to_disk(session);
            #[cfg(test)]
            if !self.in_memory {
                save_session_to_disk(session);
            }
        }
    }

    /// Get all sessions (newest first).
    pub fn sessions(&self) -> impl Iterator<Item = &Session> {
        self.sessions.iter().rev()
    }

    /// Get a specific session by ID.
    pub fn get_session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// Clear (remove) a specific session.
    pub fn clear_session(&mut self, id: SessionId) {
        self.sessions.retain(|s| s.id != id);
        #[cfg(not(test))]
        {
            let path = sessions_dir().join(format!("{}.json", id));
            let _ = std::fs::remove_file(path);
        }
        #[cfg(test)]
        if !self.in_memory {
            let path = sessions_dir().join(format!("{}.json", id));
            let _ = std::fs::remove_file(path);
        }
    }

    /// Total number of sessions.
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Create an empty in-memory manager (no disk I/O). For tests only.
    #[cfg(test)]
    fn new_empty() -> Self {
        Self {
            sessions: Vec::new(),
            next_id: 1,
            in_memory: true,
        }
    }
}

/// Persist a single session to disk.
fn save_session_to_disk(session: &Session) {
    let dir = sessions_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        log::warn!("Failed to create sessions directory: {:?}", dir);
        return;
    }
    let path = dir.join(format!("{}.json", session.id));
    match serde_json::to_string(session) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log::warn!("Failed to write session {:?}: {}", path, e);
            }
        }
        Err(e) => log::warn!("Failed to serialize session {}: {}", session.id, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::PromptSegment;
    use std::time::Instant;

    fn dummy_prompt() -> PromptInfo {
        PromptInfo {
            segments: vec![PromptSegment {
                text: "/home/user".into(),
                fg: (200, 200, 200),
                kind: crate::prompt::SegmentKind::Cwd,
            }],
            diff_additions: 0,
            diff_deletions: 0,
        }
    }

    fn dummy_block(cmd: &str) -> CommandBlock {
        CommandBlock {
            prompt: dummy_prompt(),
            command: cmd.to_string(),
            output: vec![],
            started: Instant::now(),
            duration: Some(Duration::from_millis(100)),
            selected: false,
            thinking: false,
            is_error: false,
            exit_code: Some(0),
            pending_line: None,
            checkpoint_at_start: 0,
            restored: false,
            agent_step: None,
        }
    }

    #[test]
    fn create_and_record() {
        let mut mgr = SessionManager::new_empty();
        let id = mgr.create_session();
        assert_eq!(id, 1);
        assert_eq!(mgr.count(), 1);

        let block = dummy_block("ls -la");
        let prompt = dummy_prompt();
        mgr.record_block(id, &block, &prompt);

        let session = mgr.get_session(id).unwrap();
        assert_eq!(session.entries.len(), 1);
        assert_eq!(session.entries[0].command, "ls -la");
        assert_eq!(session.entries[0].cwd, "/home/user");
    }

    #[test]
    fn display_title_from_first_command() {
        let mut mgr = SessionManager::new_empty();
        let id = mgr.create_session();
        mgr.record_block(id, &dummy_block("cargo build"), &dummy_prompt());
        let session = mgr.get_session(id).unwrap();
        assert_eq!(session.display_title(), "cargo build");
    }

    #[test]
    fn clear_session() {
        let mut mgr = SessionManager::new_empty();
        let id1 = mgr.create_session();
        let id2 = mgr.create_session();
        assert_eq!(mgr.count(), 2);

        mgr.clear_session(id1);
        assert_eq!(mgr.count(), 1);
        assert!(mgr.get_session(id1).is_none());
        assert!(mgr.get_session(id2).is_some());
    }

    #[test]
    fn sessions_newest_first() {
        let mut mgr = SessionManager::new_empty();
        let id1 = mgr.create_session();
        let id2 = mgr.create_session();
        let ids: Vec<_> = mgr.sessions().map(|s| s.id).collect();
        assert_eq!(ids, vec![id2, id1]);
    }
}
