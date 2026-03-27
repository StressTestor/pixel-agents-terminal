// Transcript file watcher

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::transcript::{parse_transcript_line, TranscriptEvent};

pub struct TranscriptReader {
    path: PathBuf,
    offset: u64,
    partial_line: String,
}

impl TranscriptReader {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            partial_line: String::new(),
        }
    }

    pub fn read_new_events(&mut self) -> Vec<TranscriptEvent> {
        let mut file = match File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return vec![];
        }

        let mut new_content = String::new();
        if file.read_to_string(&mut new_content).is_err() {
            return vec![];
        }

        // Update offset to current end of file
        match file.seek(SeekFrom::Current(0)) {
            Ok(pos) => self.offset = pos,
            Err(_) => return vec![],
        }

        // Prepend any buffered partial line
        let content = if self.partial_line.is_empty() {
            new_content
        } else {
            let mut combined = std::mem::take(&mut self.partial_line);
            combined.push_str(&new_content);
            combined
        };

        // If content doesn't end with '\n', the last segment is partial
        let ends_with_newline = content.ends_with('\n');
        let mut segments: Vec<&str> = content.split('\n').collect();

        if !ends_with_newline {
            // Last segment is incomplete — buffer it
            if let Some(partial) = segments.pop() {
                self.partial_line = partial.to_string();
            }
        } else {
            // Remove the trailing empty string from the split
            if segments.last() == Some(&"") {
                segments.pop();
            }
        }

        segments
            .into_iter()
            .filter_map(|line| parse_transcript_line(line))
            .collect()
    }
}

/// Returns true if `path` is under `allowed_root`.
/// Uses canonicalize when possible; falls back to string prefix check when the
/// path doesn't exist yet (e.g. a file about to be created).
pub fn is_safe_path(path: &Path, allowed_root: &Path) -> bool {
    match (path.canonicalize(), allowed_root.canonicalize()) {
        (Ok(resolved_path), Ok(resolved_root)) => {
            resolved_path.starts_with(&resolved_root)
        }
        _ => {
            // Canonicalize failed (path may not exist yet). Fall back to
            // string-prefix comparison on the raw paths.
            path.to_string_lossy()
                .starts_with(allowed_root.to_string_lossy().as_ref())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const TURN_END_LINE: &str =
        r#"{"type":"system","subtype":"turn_duration","durationMs":100}"#;
    const TOOL_USE_LINE: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Read","input":{}}]}}"#;

    #[test]
    fn test_read_complete_line() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "{}", TURN_END_LINE).unwrap();
        tmp.flush().unwrap();

        let mut reader = TranscriptReader::new(tmp.path().to_path_buf());
        let events = reader.read_new_events();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], TranscriptEvent::TurnEnd);
    }

    #[test]
    fn test_read_partial_then_complete() {
        let mut tmp = NamedTempFile::new().unwrap();

        // Write partial line (no newline at end)
        write!(tmp, "{}", TURN_END_LINE).unwrap();
        tmp.flush().unwrap();

        let mut reader = TranscriptReader::new(tmp.path().to_path_buf());

        // First read: no complete lines yet
        let events = reader.read_new_events();
        assert_eq!(events.len(), 0);

        // Write the newline to complete the line
        writeln!(tmp, "").unwrap();
        tmp.flush().unwrap();

        // Second read: should now return 1 event
        let events = reader.read_new_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], TranscriptEvent::TurnEnd);
    }

    #[test]
    fn test_read_multiple_lines_at_once() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "{}", TURN_END_LINE).unwrap();
        writeln!(tmp, "{}", TOOL_USE_LINE).unwrap();
        tmp.flush().unwrap();

        let mut reader = TranscriptReader::new(tmp.path().to_path_buf());
        let events = reader.read_new_events();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0], TranscriptEvent::TurnEnd);
        assert_eq!(
            events[1],
            TranscriptEvent::ToolStart {
                tool_name: "Read".to_string(),
                tool_use_id: "t1".to_string(),
            }
        );
    }

    #[test]
    fn test_safe_path_rejects_escape() {
        let dangerous = Path::new("/etc/passwd");
        let root = Path::new("/Users/test/.claude");
        assert!(!is_safe_path(dangerous, root));
    }

    #[test]
    fn test_safe_path_accepts_valid() {
        // Use tempfile to get a real path that canonicalize can resolve
        let tmp_dir = tempfile::tempdir().unwrap();
        let root = tmp_dir.path();
        let child = root.join("sessions").join("abc.jsonl");
        // child doesn't need to exist — fallback string prefix handles it
        assert!(is_safe_path(&child, root));
    }
}
