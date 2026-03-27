// JSONL transcript parser

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptEvent {
    ToolStart { tool_name: String, tool_use_id: String },
    ToolEnd { tool_use_id: String },
    PermissionPrompt { tool_use_id: String },
    TurnEnd,
}

pub fn parse_transcript_line(line: &str) -> Option<TranscriptEvent> {
    if line.is_empty() {
        return None;
    }

    let v: Value = serde_json::from_str(line).ok()?;

    let record_type = v.get("type")?.as_str()?;

    match record_type {
        "assistant" => {
            let content = v
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())?;

            for item in content {
                if item.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    let id = item.get("id")?.as_str()?;
                    let name = item.get("name")?.as_str()?;

                    if name == "AskUserQuestion" {
                        return Some(TranscriptEvent::PermissionPrompt {
                            tool_use_id: id.to_string(),
                        });
                    } else {
                        return Some(TranscriptEvent::ToolStart {
                            tool_name: name.to_string(),
                            tool_use_id: id.to_string(),
                        });
                    }
                }
            }
            None
        }

        "user" => {
            let content = v
                .get("message")
                .and_then(|m| m.get("content"))?;

            // string content = plain user message, ignore
            let content_arr = content.as_array()?;

            for item in content_arr {
                if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                    let tool_use_id = item.get("tool_use_id")?.as_str()?;
                    return Some(TranscriptEvent::ToolEnd {
                        tool_use_id: tool_use_id.to_string(),
                    });
                }
            }
            None
        }

        "system" => {
            let subtype = v.get("subtype").and_then(|s| s.as_str())?;
            if subtype == "turn_duration" {
                Some(TranscriptEvent::TurnEnd)
            } else {
                None
            }
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_use_write() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_123","name":"Write","input":{}}]}}"#;
        assert_eq!(
            parse_transcript_line(line),
            Some(TranscriptEvent::ToolStart {
                tool_name: "Write".to_string(),
                tool_use_id: "toolu_123".to_string(),
            })
        );
    }

    #[test]
    fn test_tool_use_read() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_456","name":"Read","input":{}}]}}"#;
        assert_eq!(
            parse_transcript_line(line),
            Some(TranscriptEvent::ToolStart {
                tool_name: "Read".to_string(),
                tool_use_id: "toolu_456".to_string(),
            })
        );
    }

    #[test]
    fn test_tool_result() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_123","content":"ok","is_error":false}]}}"#;
        assert_eq!(
            parse_transcript_line(line),
            Some(TranscriptEvent::ToolEnd {
                tool_use_id: "toolu_123".to_string(),
            })
        );
    }

    #[test]
    fn test_ask_user_question() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_789","name":"AskUserQuestion","input":{}}]}}"#;
        assert_eq!(
            parse_transcript_line(line),
            Some(TranscriptEvent::PermissionPrompt {
                tool_use_id: "toolu_789".to_string(),
            })
        );
    }

    #[test]
    fn test_system_turn_duration() {
        let line = r#"{"type":"system","subtype":"turn_duration","durationMs":5000}"#;
        assert_eq!(parse_transcript_line(line), Some(TranscriptEvent::TurnEnd));
    }

    #[test]
    fn test_malformed_json() {
        let line = "{broken";
        assert_eq!(parse_transcript_line(line), None);
    }

    #[test]
    fn test_empty_line() {
        let line = "";
        assert_eq!(parse_transcript_line(line), None);
    }

    #[test]
    fn test_user_message_ignored() {
        // user record with string content (plain message) -> None
        let line = r#"{"type":"user","message":{"role":"user","content":"hello there"}}"#;
        assert_eq!(parse_transcript_line(line), None);
    }

    #[test]
    fn test_agent_tool_use() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_abc","name":"Agent","input":{}}]}}"#;
        assert_eq!(
            parse_transcript_line(line),
            Some(TranscriptEvent::ToolStart {
                tool_name: "Agent".to_string(),
                tool_use_id: "toolu_abc".to_string(),
            })
        );
    }

    #[test]
    fn test_unknown_tool() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_xyz","name":"FutureTool","input":{}}]}}"#;
        assert_eq!(
            parse_transcript_line(line),
            Some(TranscriptEvent::ToolStart {
                tool_name: "FutureTool".to_string(),
                tool_use_id: "toolu_xyz".to_string(),
            })
        );
    }
}
