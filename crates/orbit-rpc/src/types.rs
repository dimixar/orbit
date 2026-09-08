//! Typed model of the pi CLI RPC protocol (`pi --mode rpc`).
//!
//! Protocol reference: `packages/coding-agent/docs/rpc.md` in the pi repo.
//! Framing is strict JSONL: LF-only record delimiter, optional trailing `\r`.
//!
//! This module is intentionally permissive: every envelope is parsed loosely
//! and unknown fields/variants flow through as raw JSON so forward protocol
//! changes never break the client. Full typing of every message shape is
//! graduated into P2 once real event dumps are in from live runs.

use serde_json::Value;

// ───────────────────────────────────────────────────────────────────────────
// Commands (sent to stdin, one JSON line each)
// ───────────────────────────────────────────────────────────────────────────

/// A command envelope. `id` is optional for correlation on the wire, but the
/// client always adds one so every command gets a matched response.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Command {
    pub id: String,
    #[serde(flatten)]
    pub body: CommandBody,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandBody {
    /// Send a user prompt. `images` are `{type:"image", data: base64, mimeType}`.
    Prompt {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        images: Option<Vec<Value>>,
        #[serde(rename = "streamingBehavior", skip_serializing_if = "Option::is_none")]
        streaming_behavior: Option<String>,
    },
    /// Interrupt the current assistant turn; queued messages keep running.
    Abort,
    /// Drop everything queued for the current session.
    ClearQueue,
    /// Inject a follow-up message into the running turn without aborting it
    /// (Waku's steer: the agent sees it as soon as the current step ends).
    Steer {
        message: String,
    },
    /// Create a fresh session (new session id/file returned in the response).
    NewSession,
    /// Load a different session file.
    SwitchSession {
        #[serde(rename = "sessionPath")]
        session_path: String,
    },
    GetState,
    GetMessages,
    GetAvailableModels,
    SetModel {
        #[serde(rename = "modelId")]
        model_id: String,
        provider: String,
    },
    CycleModel,
    GetAvailableThinkingLevels,
    SetThinkingLevel {
        level: String,
    },
    CycleThinkingLevel,
    GetCommands,
    /// Token totals + current context-window usage for the open session.
    GetSessionStats,
    /// Message list (with `entryId`s) used to build a fork point.
    GetForkMessages,
    /// Rewind the session to just before `entry_id` (drops later turns).
    Fork {
        #[serde(rename = "entryId")]
        entry_id: String,
    },
    /// Duplicate the session as-is (fork with nothing removed).
    #[serde(rename = "clone")]
    CloneSession,

    /// Anything the typed enum does not cover yet; sent verbatim.
    Raw(Value),
}

impl Command {
    pub fn new(id: impl Into<String>, body: CommandBody) -> Self {
        Self {
            id: id.into(),
            body,
        }
    }

    /// Serialize to the JSONL wire form (single line, no trailing newline).
    pub fn to_wire(&self) -> anyhow::Result<String> {
        let value = match &self.body {
            CommandBody::Raw(payload) => {
                let mut value = payload.clone();
                // A payload may carry its own id (extension UI responses
                // reference the request's id) — only stamp ours when absent.
                if value.get("id").is_none() {
                    value["id"] = Value::String(self.id.clone());
                }
                value
            }
            _ => serde_json::to_value(self)?,
        };
        serde_json::to_string(&value).map_err(Into::into)
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Events (streamed to stdout as JSON lines)
// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Event {
    /// Reply to a command (`{"type":"response","id":…,"command":…,"success":…}`).
    Response {
        id: String,
        command: String,
        success: bool,
        data: Option<Value>,
        error: Option<String>,
    },

    /// Streaming update for the assistant message currently being produced.
    MessageUpdate {
        usage: Option<Value>,
        assistant: Option<AssistantMessageEvent>,
    },

    AgentStart,
    AgentEnd {
        will_retry: bool,
    },
    AgentSettled,
    TurnStart,
    TurnEnd {
        value: Value,
    },
    MessageStart {
        value: Value,
    },
    MessageEnd {
        value: Value,
    },
    ToolExecutionStart {
        value: Value,
    },
    ToolExecutionUpdate {
        value: Value,
    },
    ToolExecutionEnd {
        value: Value,
    },
    BashExecutionUpdate {
        value: Value,
    },
    QueueUpdate {
        value: Value,
    },
    CompactionStart {
        value: Value,
    },
    CompactionEnd {
        value: Value,
    },
    AutoRetryStart {
        value: Value,
    },
    AutoRetryEnd {
        value: Value,
    },

    /// pi renamed the open session (`name` is `null` when cleared). Waku
    /// forwards this as an automatic session title.
    SessionInfoChanged {
        name: Option<String>,
    },
    ExtensionError {
        value: Value,
    },

    /// User interaction request (question dialogs, status widgets, …).
    /// Dialog methods (`select`/`confirm`/`input`/`editor`) expect an
    /// `extension_ui_response` on stdin with the matching `id`.
    ExtensionUiRequest {
        id: String,
        method: String,
        value: Value,
    },

    /// The pi process exited (reader loop hit EOF).
    ProcessExited,

    /// Everything we have not typed yet; kept verbatim.
    Unknown(Value),
}

/// The per-message delta carried inside `message_update`.
#[derive(Debug, Clone)]
pub enum AssistantMessageEvent {
    TextDelta {
        delta: String,
    },
    ThinkingDelta {
        delta: String,
    },
    ToolcallStart {
        value: Value,
    },
    ToolcallDelta {
        value: Value,
    },
    ToolcallEnd {
        value: Value,
    },
    /// e.g. `text_content_updated`, finished-message snapshots, new shapes.
    Other {
        kind: String,
        value: Value,
    },
}

impl Event {
    /// Parse one JSONL line into an [`Event`]. Non-object or empty lines are
    /// treated as unknown rather than fatal, matching pi's own tolerance.
    pub fn parse_line(line: &str) -> Event {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Event::Unknown(Value::String(line.to_string()));
        };
        Self::from_value(value)
    }

    pub fn from_value(value: Value) -> Event {
        let Some(kind) = value.get("type").and_then(Value::as_str) else {
            return Event::Unknown(value);
        };
        match kind {
            "response" => {
                let id = value
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let command = value
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let success = value
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                Event::Response {
                    id,
                    command,
                    success,
                    data: value.get("data").cloned(),
                    error: value
                        .get("error")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                }
            }
            "message_update" => {
                let assistant = value.get("assistantMessageEvent").cloned().map(|v| {
                    let ev = v.get("type").and_then(Value::as_str).unwrap_or("");
                    match ev {
                        "text_delta" => AssistantMessageEvent::TextDelta {
                            delta: v
                                .get("delta")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                        },
                        "thinking_delta" => AssistantMessageEvent::ThinkingDelta {
                            delta: v
                                .get("delta")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                        },
                        "toolcall_start" => AssistantMessageEvent::ToolcallStart { value: v },
                        "toolcall_delta" => AssistantMessageEvent::ToolcallDelta { value: v },
                        "toolcall_end" => AssistantMessageEvent::ToolcallEnd { value: v },
                        other => AssistantMessageEvent::Other {
                            kind: other.to_string(),
                            value: v,
                        },
                    }
                });
                Event::MessageUpdate {
                    usage: value.get("usage").cloned(),
                    assistant,
                }
            }
            "agent_start" => Event::AgentStart,
            "agent_end" => Event::AgentEnd {
                will_retry: value
                    .get("willRetry")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
            "agent_settled" => Event::AgentSettled,
            "turn_start" => Event::TurnStart,
            "turn_end" => Event::TurnEnd { value },
            "message_start" => Event::MessageStart { value },
            "message_end" => Event::MessageEnd { value },
            "tool_execution_start" => Event::ToolExecutionStart { value },
            "tool_execution_update" => Event::ToolExecutionUpdate { value },
            "tool_execution_end" => Event::ToolExecutionEnd { value },
            "bash_execution_update" => Event::BashExecutionUpdate { value },
            "queue_update" => Event::QueueUpdate { value },
            "compaction_start" => Event::CompactionStart { value },
            "compaction_end" => Event::CompactionEnd { value },
            "auto_retry_start" => Event::AutoRetryStart { value },
            "auto_retry_end" => Event::AutoRetryEnd { value },
            "session_info_changed" => Event::SessionInfoChanged {
                name: value
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned),
            },
            "extension_error" => Event::ExtensionError { value },
            "extension_ui_request" => Event::ExtensionUiRequest {
                id: value
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                method: value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                value,
            },
            _ => Event::Unknown(value),
        }
    }

    /// A short, single-line human description used by the dev UI and logs.
    pub fn one_line(&self) -> String {
        match self {
            Event::Response {
                command, success, ..
            } => {
                format!(
                    "response → {command} ({})",
                    if *success { "ok" } else { "err" }
                )
            }
            Event::MessageUpdate { assistant, .. } => match assistant {
                Some(AssistantMessageEvent::TextDelta { delta }) => {
                    format!("text+ {}", delta_summary(delta))
                }
                Some(AssistantMessageEvent::ThinkingDelta { delta }) => {
                    format!("think+ {}", trim(delta, 48))
                }
                Some(AssistantMessageEvent::ToolcallStart { value }) => {
                    format!("tool→ {}", value)
                }
                Some(AssistantMessageEvent::ToolcallDelta { .. }) => "tool args…".into(),
                Some(AssistantMessageEvent::ToolcallEnd { value }) => format!("tool ✓ {value}"),
                Some(AssistantMessageEvent::Other { kind, .. }) => format!("msg:{kind}"),
                None => "message_update".into(),
            },
            Event::AgentStart => "agent_start".into(),
            Event::AgentEnd { will_retry } => format!("agent_end (retry={will_retry})"),
            Event::SessionInfoChanged { name } => {
                format!("session rename → {}", name.as_deref().unwrap_or("(cleared)"))
            }
            Event::AgentSettled => "agent_settled ✓".into(),
            Event::ExtensionUiRequest { method, .. } => format!("ui request: {method}"),
            Event::ProcessExited => "pi exited".into(),
            Event::Unknown(v) => trim(&v.to_string(), 80),
            other => trim(&format!("{other:?}"), 80),
        }
    }
}

fn delta_summary(delta: &str) -> String {
    let t = trim(delta, 48);
    if t.is_empty() {
        "(empty)".into()
    } else {
        t
    }
}

/// Current context-window snapshot from `get_session_stats`.
///
/// `tokens` / `percent` are `None` immediately after compaction until the
/// next assistant response establishes a fresh baseline. `contextUsage` itself
/// is omitted when no model (or no window) is set — then [`from_stats`]
/// returns `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextUsage {
    pub tokens: Option<u64>,
    pub context_window: u64,
    pub percent: Option<f64>,
}

impl ContextUsage {
    /// Parse the `contextUsage` object on a `get_session_stats` payload.
    pub fn from_stats(data: &Value) -> Option<Self> {
        let usage = data.get("contextUsage")?;
        if usage.is_null() {
            return None;
        }
        let context_window = json_u64(usage.get("contextWindow")?)?;
        if context_window == 0 {
            return None;
        }
        let tokens = usage.get("tokens").and_then(json_u64_or_null);
        let percent = match usage.get("percent").and_then(json_f64_or_null) {
            Some(p) => Some(p),
            None => tokens.map(|t| (t as f64 / context_window as f64) * 100.0),
        };
        Some(Self {
            tokens,
            context_window,
            percent,
        })
    }

    /// Fill fraction for a meter, 0..=1. `None` when the estimate is unknown.
    pub fn fraction(&self) -> Option<f32> {
        self.percent.map(|p| (p / 100.0).clamp(0.0, 1.0) as f32)
    }
}

fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
        .or_else(|| value.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64))
}

fn json_f64(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_u64().map(|n| n as f64))
}

fn json_u64_or_null(value: &Value) -> Option<u64> {
    if value.is_null() {
        None
    } else {
        json_u64(value)
    }
}

fn json_f64_or_null(value: &Value) -> Option<f64> {
    if value.is_null() {
        None
    } else {
        json_f64(value)
    }
}

fn trim(s: &str, max: usize) -> String {
    let s = s.replace('\n', "\\n");
    if s.chars().count() > max {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    } else {
        s
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Tests — against the real wire captures from the P0 probe
// ───────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_get_state_response() {
        // Captured live from `pi --mode rpc` + get_state.
        let line = r#"{"id":"probe-1","type":"response","command":"get_state","success":true,"data":{"model":{"id":"glm-5.3-flash:cloud","name":"glm-5.3-flash:cloud","api":"openai-completions","provider":"ollama","baseUrl":"http://127.0.0.1:11434/v1","reasoning":true},"thinkingLevel":"high","isStreaming":false,"sessionId":"01a07aa2-d867-753c-98c3-0518c4879639","messageCount":0}}"#;
        let ev = Event::parse_line(line);
        match ev {
            Event::Response {
                id,
                command,
                success,
                data,
                ..
            } => {
                assert_eq!(id, "probe-1");
                assert_eq!(command, "get_state");
                assert!(success);
                let data = data.expect("data present");
                assert_eq!(data["thinkingLevel"], "high");
                assert_eq!(data["model"]["provider"], "ollama");
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    #[test]
    fn parses_message_update_textdelta() {
        let line = r#"{"type":"message_update","usage":{"input":1,"output":2},"assistantMessageEvent":{"type":"text_delta","delta":"Hello"}}"#;
        match Event::parse_line(line) {
            Event::MessageUpdate {
                assistant: Some(AssistantMessageEvent::TextDelta { delta }),
                ..
            } => {
                assert_eq!(delta, "Hello")
            }
            other => panic!("expected text delta, got {other:?}"),
        }
    }

    #[test]
    fn command_wire_serializes_with_id() {
        let cmd = Command::new(
            "req-1",
            CommandBody::Prompt {
                message: "hello".into(),
                images: None,
                streaming_behavior: None,
            },
        );
        let wire = cmd.to_wire().unwrap();
        // JSON objects are unordered; compare semantics, not key order.
        let parsed: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(parsed["id"], "req-1");
        assert_eq!(parsed["type"], "prompt");
        assert_eq!(parsed["message"], "hello");
        assert_eq!(parsed.get("images"), None);
    }

    #[test]
    fn steer_serializes_with_message() {
        let wire = Command::new("st1", CommandBody::Steer {
            message: "use tokio instead".into(),
        })
        .to_wire()
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(parsed["type"], "steer");
        assert_eq!(parsed["message"], "use tokio instead");
    }

    #[test]
    fn fork_family_serializes_like_pi_expects() {
        let fork = Command::new("f1", CommandBody::Fork {
            entry_id: "turn-2".into(),
        })
        .to_wire()
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&fork).unwrap();
        assert_eq!(parsed["type"], "fork");
        assert_eq!(parsed["entryId"], "turn-2");

        let clone = Command::new("f2", CommandBody::CloneSession)
            .to_wire()
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&clone).unwrap();
        assert_eq!(parsed["type"], "clone");
    }

    #[test]
    fn session_rename_parses_and_trims() {
        assert!(matches!(
            Event::parse_line(r#"{"type":"session_info_changed","name":"  Named by pi  "}"#),
            Event::SessionInfoChanged { name: Some(n) } if n == "Named by pi"
        ));
        assert!(matches!(
            Event::parse_line(r#"{"type":"session_info_changed","name":null}"#),
            Event::SessionInfoChanged { name: None }
        ));
    }

    #[test]
    fn tolerates_unknown_events() {
        let ev = Event::parse_line(r#"{"type":"brand_new_shape","thing":1}"#);
        assert!(matches!(ev, Event::Unknown(_)));
    }

    #[test]
    fn framing_strips_carriage_return() {
        let line = "{\"type\":\"agent_start\"}\r";
        assert!(matches!(Event::parse_line(line), Event::AgentStart));
    }

    #[test]
    fn get_session_stats_serializes() {
        let wire = Command::new("s1", CommandBody::GetSessionStats)
            .to_wire()
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(parsed["id"], "s1");
        assert_eq!(parsed["type"], "get_session_stats");
    }

    #[test]
    fn context_usage_from_stats_payload() {
        let data: serde_json::Value = serde_json::from_str(
            r#"{"contextUsage":{"tokens":60000,"contextWindow":200000,"percent":30}}"#,
        )
        .unwrap();
        let usage = ContextUsage::from_stats(&data).expect("contextUsage");
        assert_eq!(usage.tokens, Some(60_000));
        assert_eq!(usage.context_window, 200_000);
        assert_eq!(usage.percent, Some(30.0));
        assert!((usage.fraction().unwrap() - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn context_usage_null_after_compaction() {
        let data: serde_json::Value = serde_json::from_str(
            r#"{"contextUsage":{"tokens":null,"contextWindow":200000,"percent":null}}"#,
        )
        .unwrap();
        let usage = ContextUsage::from_stats(&data).expect("window still known");
        assert_eq!(usage.tokens, None);
        assert_eq!(usage.percent, None);
        assert_eq!(usage.fraction(), None);
    }

    #[test]
    fn context_usage_omitted_without_model() {
        let data = serde_json::json!({"tokens":{"total":10}});
        assert!(ContextUsage::from_stats(&data).is_none());
    }
}
