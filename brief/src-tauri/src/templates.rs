//! Minimal system prompts for Ollama summarization (full template library: BRIEF-P2-002).

/// Returns a system prompt that asks for structured JSON matching [crate::types::MeetingOutput] fields.
pub fn get_system_prompt(meeting_type: &str) -> String {
    let context = match meeting_type {
        "legal" => "This is a legal or compliance-oriented meeting.",
        "internal" => "This is an internal team meeting.",
        "consulting" | _ => "This is a consulting or client-facing meeting.",
    };

    format!(
        r#"You are a meeting assistant. {context}

Read the transcript and produce a single JSON object with these keys (use only these keys):
- "summary_short": string, concise summary (2-4 sentences)
- "topics": array of objects with string fields as appropriate (e.g. title, summary)
- "decisions": array of objects (e.g. description, context)
- "action_items": array of objects (e.g. description, owner, due_date, priority)
- "follow_up_draft": object (e.g. subject, greeting, body, closing, full_text) or empty object {{}}
- "participants_mentioned": array of strings (names or roles mentioned)

Output valid JSON only, no markdown fences or commentary."#,
        context = context
    )
}
