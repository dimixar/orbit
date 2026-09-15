//! `ask_user_question` — the structured questionnaire tool.
//!
//! The tool is an extension (`@juicesharp/rpiv-ask-user-question`). A terminal
//! renders it as a tabbed overlay; in RPC mode (Orbit) it cannot, so it walks
//! the questionnaire through pi's `select` / `input` dialog primitives
//! instead. Orbit answers those on an inline panel above the composer — no
//! scrim modal — and renders the finished tool call as a question card in the
//! transcript.
//!
//! This module is pure data: parsing the tool arguments (`questions`) and
//! reading the answer envelope back. The panel state lives in
//! [`crate::app::ask`]; rendering lives in `app/view.rs` and
//! `transcript_view.rs`.

use gpui::Entity;
use serde_json::Value;

use crate::composer::ComposerInput;

/// One author-defined option of a question.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AskOption {
    pub label: String,
    pub description: String,
    /// Optional markdown preview (mockups, code, diagrams) shown for the
    /// focused option.
    pub preview: Option<String>,
}

/// One question in an `ask_user_question` call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AskQuestion {
    /// Short chip/tag shown next to the question (≤16 chars).
    pub header: String,
    /// The full question text.
    pub question: String,
    /// Whether more than one option may be selected.
    pub multi_select: bool,
    pub options: Vec<AskOption>,
}

/// Which primitive the live panel is answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskMode {
    /// Single-select: option rows plus a "Type something." escape.
    Select,
    /// Multi-select: toggle rows plus a Continue action.
    Multi,
    /// Free text (a single-select custom answer, or a multi custom answer).
    Custom,
}

/// The live questionnaire panel rendered above the composer. It answers the
/// extension's `select` / `input` requests in place of the scrim modal. The
/// rich structure (headers, descriptions) comes from the tool arguments; the
/// response value only needs to encode the chosen index, which is exactly
/// what the extension parses back.
pub struct AskPrompt {
    /// RPC id of the live `extension_ui_request`.
    pub id: String,
    pub mode: AskMode,
    /// Index of the question being answered (for the "N of M" hint).
    pub question_ix: usize,
    pub total: usize,
    pub header: String,
    pub question: String,
    pub options: Vec<AskOption>,
    /// Single-select questions append a "Type something." row (the last row).
    pub custom_row: bool,
    /// Multi-select toggles, one per option.
    pub checked: Vec<bool>,
    /// Highlighted row (0..=options.len(); the last is custom / Continue).
    pub highlighted: usize,
    /// A response was sent; the panel waits for the next request or the tool
    /// to end. Clicks are ignored while set.
    pub submitted: bool,
    /// Text field for `Custom` (and the custom-answer field for `Multi`).
    pub input: Option<Entity<ComposerInput>>,
}

impl AskPrompt {
    /// The number of selectable rows (options plus the trailing custom /
    /// Continue row when the mode has one).
    pub fn row_count(&self) -> usize {
        let extra = match self.mode {
            AskMode::Select => usize::from(self.custom_row),
            AskMode::Multi => 1, // Continue
            AskMode::Custom => 0,
        };
        self.options.len() + extra
    }
}

/// Tool names that carry the structured questionnaire. `ask_user_question` is
/// the extension's name; the shorter aliases cover older/renamed registrations
/// and the analytics classifier's vocabulary.
pub fn is_ask_tool(name: &str) -> bool {
    matches!(name, "ask_user_question" | "ask_question" | "ask_user")
}

/// Parse the `questions` array out of an `ask_user_question` tool's arguments.
/// Tolerant by design: a missing or malformed payload yields an empty list
/// (the caller then leaves the request to the generic dialog path), never an
/// error.
pub fn questions_from_args(args: Option<&Value>) -> Vec<AskQuestion> {
    let Some(list) = args
        .and_then(|args| args.get("questions"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    list.iter().filter_map(question_from_value).collect()
}

fn question_from_value(value: &Value) -> Option<AskQuestion> {
    // A question without text is not renderable; skip it rather than show a
    // blank card.
    let question = value.get("question").and_then(Value::as_str)?.to_string();
    let header = value
        .get("header")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let multi_select = value
        .get("multiSelect")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let options = value
        .get("options")
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(option_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(AskQuestion {
        header,
        question,
        multi_select,
        options,
    })
}

fn option_from_value(value: &Value) -> Option<AskOption> {
    let label = value.get("label").and_then(Value::as_str)?.to_string();
    let description = value
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let preview = value
        .get("preview")
        .and_then(Value::as_str)
        .filter(|preview| !preview.is_empty())
        .map(str::to_string);
    Some(AskOption {
        label,
        description,
        preview,
    })
}

/// Find the question a live `extension_ui_request` belongs to. The extension
/// prefixes the request title with `[Header] ` and appends folded preview
/// blocks, but always embeds the full question text — so a substring match
/// against the authored questions is exact enough to pick the right one.
pub fn question_index_for_title(questions: &[AskQuestion], title: &str) -> Option<usize> {
    questions
        .iter()
        .position(|question| title.contains(question.question.as_str()))
}

/// Whether the tool result is the extension's "declined" envelope. The
/// extension uses one canonical string for every way a questionnaire can end
/// without an answer.
pub fn is_declined(output: Option<&Value>) -> bool {
    matches!(output, Some(Value::String(text)) if text.contains("declined to answer"))
}

/// The answer envelope's raw text, when the tool completed with answers.
pub fn answer_envelope(output: Option<&Value>) -> Option<&str> {
    match output {
        Some(Value::String(text)) if text.starts_with("User has answered your questions:") => {
            Some(text.as_str())
        }
        _ => None,
    }
}

/// The comma-separated labels (or typed text) inside one answer.
fn answer_parts(answer: &str) -> impl Iterator<Item = &str> {
    answer
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

/// Whether one of the answer's parts is `label`. Multi-select answers arrive
/// comma-joined (`"A, B"`), so a whole-string comparison would miss them.
pub fn answer_includes_label(answer: &str, label: &str) -> bool {
    answer_parts(answer).any(|part| part == label)
}

/// Whether every part of the answer is one of the question's option labels
/// (i.e. the user picked options rather than typing a custom answer).
pub fn answer_is_option_selection(question: &AskQuestion, answer: &str) -> bool {
    let mut any = false;
    for part in answer_parts(answer) {
        any = true;
        if !question.options.iter().any(|option| option.label == part) {
            return false;
        }
    }
    any
}

/// The answer the envelope records for one question, if any.
///
/// The envelope is `User has answered your questions: "Q"="A". "Q2"="A2". …`,
/// where `A` is an option label (multi answers comma-joined) or the typed
/// custom text. Anchoring on the quoted question text reads exactly that
/// answer back, so the transcript can show what was chosen instead of asking
/// the reader to infer it from the options.
pub fn envelope_answer<'a>(envelope: &'a str, question: &str) -> Option<&'a str> {
    let needle = format!("\"{question}\"=\"");
    let start = envelope.find(&needle)? + needle.len();
    let rest = &envelope[start..];
    // The answer runs to the closing quote that ends the segment — `". `
    // before the next segment, or `".` at the very end.
    let end = rest
        .find("\". ")
        .or_else(|| rest.find("\"."))
        .unwrap_or(rest.len());
    let answer = rest[..end].trim();
    (!answer.is_empty()).then_some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_questions_and_options() {
        let questions = questions_from_args(Some(&json!({
            "questions": [{
                "header": "Auth method",
                "question": "Which auth should we use?",
                "multiSelect": true,
                "options": [
                    {"label": "OAuth", "description": "Delegated login."},
                    {"label": "API key", "description": "Static token.", "preview": "KEY=…"},
                ],
            }],
        })));
        assert_eq!(questions.len(), 1);
        let question = &questions[0];
        assert_eq!(question.header, "Auth method");
        assert_eq!(question.question, "Which auth should we use?");
        assert!(question.multi_select);
        assert_eq!(question.options.len(), 2);
        assert_eq!(question.options[1].preview.as_deref(), Some("KEY=…"));
    }

    #[test]
    fn malformed_payloads_degrade_to_empty() {
        assert!(questions_from_args(None).is_empty());
        assert!(questions_from_args(Some(&json!({"questions": "nope"}))).is_empty());
        // A question with no text is skipped, not rendered blank.
        assert!(questions_from_args(Some(&json!({
            "questions": [{"header": "X", "options": []}]
        })))
        .is_empty());
    }

    #[test]
    fn matches_a_request_title_to_its_question() {
        let questions = vec![
            AskQuestion {
                header: "A".into(),
                question: "First question?".into(),
                ..Default::default()
            },
            AskQuestion {
                header: "B".into(),
                question: "Second question?".into(),
                ..Default::default()
            },
        ];
        let title = "[B] Second question?\n\n--- 1. Preview ---\nbody";
        assert_eq!(question_index_for_title(&questions, title), Some(1));
        assert_eq!(question_index_for_title(&questions, "unrelated"), None);
    }

    #[test]
    fn reads_the_answer_envelope() {
        let answered = json!("User has answered your questions: \"Q\"=\"OAuth\". You can now continue with the user's answers in mind.");
        let envelope = answer_envelope(Some(&answered)).unwrap();
        assert_eq!(envelope_answer(envelope, "Q"), Some("OAuth"));
        assert_eq!(envelope_answer(envelope, "Other"), None);
        assert!(answer_includes_label("OAuth", "OAuth"));
        assert!(answer_includes_label("A, B", "B"));
        assert!(!answer_includes_label("A, B", "C"));
        assert!(is_declined(Some(&json!(
            "User declined to answer questions"
        ))));
        assert!(!is_declined(Some(&answered)));
    }

    #[test]
    fn tells_option_selections_from_custom_answers() {
        let question = AskQuestion {
            options: vec![
                AskOption {
                    label: "A".into(),
                    ..Default::default()
                },
                AskOption {
                    label: "B".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert!(answer_is_option_selection(&question, "A"));
        assert!(answer_is_option_selection(&question, "A, B"));
        assert!(!answer_is_option_selection(&question, "something else"));
        assert!(!answer_is_option_selection(&question, "A, something else"));
    }
}
