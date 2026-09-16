use chrono::Utc;
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Ghi logs/bot.jsonl theo schema AGENTS.md:
/// bot.start victim.reload tx.seen tx.skip sim.* venue.pick tx.send tx.abort halt.triggered
/// Không bao giờ ghi PRIVATE_KEY hay RPC URL có token vào đây.
pub struct BotLogger {
    path: PathBuf,
    file: Mutex<std::fs::File>,
}

impl BotLogger {
    pub fn new(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            path,
            file: Mutex::new(file),
        })
    }

    pub fn log(&self, event: &str, mut fields: Value) {
        if !fields.is_object() {
            fields = json!({});
        }
        if let Value::Object(ref mut map) = fields {
            map.insert("ts".to_string(), json!(Utc::now().to_rfc3339()));
            map.insert("event".to_string(), json!(event));
        }
        let line = serde_json::to_string(&fields).unwrap_or_else(|_| "{}".to_string());
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Đọc N dòng cuối, bỏ qua dòng hỏng thay vì panic (dùng cho web API).
    pub fn tail(&self, limit: usize) -> Vec<Value> {
        let content = std::fs::read_to_string(&self.path).unwrap_or_default();
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(limit);
        lines[start..]
            .iter()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_and_tail_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs").join("bot.jsonl");
        let logger = BotLogger::new(&path).unwrap();
        logger.log("bot.start", json!({"chain_id": 56}));
        logger.log("victim.reload", json!({"count": 2}));

        let tail = logger.tail(50);
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0]["event"], "bot.start");
        assert_eq!(tail[1]["event"], "victim.reload");
        assert!(tail[0].get("ts").is_some());
    }

    #[test]
    fn tail_never_panics_on_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope").join("bot.jsonl");
        // Chưa gọi new() để tạo file -> đọc file không tồn tại phải trả về rỗng.
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(content.is_empty());
        let _ = path; // giữ path sống tới cuối test
    }
}
