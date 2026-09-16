use serde::Serialize;
use std::path::PathBuf;

/// Theo AGENTS.md mục "State / log":
/// IDLE -> WATCHING -> HIT -> SIM_LOCK -> LOGGED
/// Live: SENDING_FRONT -> SENDING_BACK
/// STOPPED --reset--> IDLE
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BotState {
    Idle,
    Watching,
    Hit,
    SimLock,
    Logged,
    SendingFront,
    SendingBack,
    Stopped,
}

impl BotState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BotState::Idle => "IDLE",
            BotState::Watching => "WATCHING",
            BotState::Hit => "HIT",
            BotState::SimLock => "SIM_LOCK",
            BotState::Logged => "LOGGED",
            BotState::SendingFront => "SENDING_FRONT",
            BotState::SendingBack => "SENDING_BACK",
            BotState::Stopped => "STOPPED",
        }
    }
}

impl Default for BotState {
    fn default() -> Self {
        BotState::Idle
    }
}

/// Quản lý 3 file điều khiển: state/halt.lock, state/disarm.req, state/reset.req.
/// Web dashboard chỉ được phép GHI các file này (không gọi signer trực tiếp).
pub struct StateFiles {
    dir: PathBuf,
}

impl StateFiles {
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn halt_lock_path(&self) -> PathBuf {
        self.dir.join("halt.lock")
    }
    pub fn disarm_req_path(&self) -> PathBuf {
        self.dir.join("disarm.req")
    }
    pub fn reset_req_path(&self) -> PathBuf {
        self.dir.join("reset.req")
    }

    pub fn is_halted(&self) -> bool {
        self.halt_lock_path().exists()
    }

    pub fn request_halt(&self) -> std::io::Result<()> {
        std::fs::write(self.halt_lock_path(), b"halt")
    }
    pub fn request_disarm(&self) -> std::io::Result<()> {
        std::fs::write(self.disarm_req_path(), b"disarm")
    }
    pub fn request_reset(&self) -> std::io::Result<()> {
        std::fs::write(self.reset_req_path(), b"reset")
    }

    pub fn clear_halt(&self) -> std::io::Result<()> {
        let p = self.halt_lock_path();
        if p.exists() {
            std::fs::remove_file(p)?;
        }
        Ok(())
    }

    pub fn take_reset_req(&self) -> std::io::Result<bool> {
        let p = self.reset_req_path();
        if p.exists() {
            std::fs::remove_file(&p)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn take_disarm_req(&self) -> std::io::Result<bool> {
        let p = self.disarm_req_path();
        if p.exists() {
            std::fs::remove_file(&p)?;
            return Ok(true);
        }
        Ok(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    Halt,
    Disarm,
    Reset,
}

impl ControlAction {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "halt" => Some(ControlAction::Halt),
            "disarm" => Some(ControlAction::Disarm),
            "reset" => Some(ControlAction::Reset),
            _ => None,
        }
    }

    pub fn apply(self, files: &StateFiles) -> std::io::Result<()> {
        match self {
            ControlAction::Halt => files.request_halt(),
            ControlAction::Disarm => files.request_disarm(),
            ControlAction::Reset => files.request_reset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halt_lock_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let files = StateFiles::new(dir.path().join("state")).unwrap();
        assert!(!files.is_halted());
        files.request_halt().unwrap();
        assert!(files.is_halted());
        files.clear_halt().unwrap();
        assert!(!files.is_halted());
    }

    #[test]
    fn control_action_parse_unknown_is_none() {
        assert!(ControlAction::parse("nuke").is_none());
        assert_eq!(ControlAction::parse("halt"), Some(ControlAction::Halt));
    }
}
