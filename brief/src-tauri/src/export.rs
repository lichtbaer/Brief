//! Meeting export as Markdown and PDF (printpdf).

use printpdf::{BuiltinFont, Mm, PdfDocument};

/// Builds Markdown from stored meeting JSON (snake_case keys).
pub fn generate_markdown(meeting: &serde_json::Value) -> String {
    let title = meeting["title"].as_str().unwrap_or("Meeting");
    let date = meeting["created_at"].as_str().unwrap_or("");
    let meeting_type = meeting["meeting_type"].as_str().unwrap_or("");
    let output = &meeting["output"];

    let mut md = format!("# {title}\n\n**Datum:** {date}  \n**Typ:** {meeting_type}\n\n");

    if let Some(summary) = output["summary_short"].as_str() {
        if !summary.is_empty() {
            md.push_str(&format!("## Zusammenfassung\n\n{summary}\n\n"));
        }
    }

    if let Some(topics) = output["topics"].as_array() {
        if !topics.is_empty() {
            md.push_str("## Besprochene Themen\n\n");
            for topic in topics {
                let t = topic["title"].as_str().unwrap_or("");
                let summary = topic["summary"].as_str().unwrap_or("");
                md.push_str(&format!("### {t}\n\n{summary}\n\n"));
            }
        }
    }

    if let Some(decisions) = output["decisions"].as_array() {
        if !decisions.is_empty() {
            md.push_str("## Entscheidungen\n\n");
            for d in decisions {
                let desc = d["description"].as_str().unwrap_or("");
                md.push_str(&format!("- {desc}\n"));
                if let Some(ctx) = d.get("context").and_then(|v| v.as_str()) {
                    if !ctx.is_empty() {
                        md.push_str(&format!("  {ctx}\n"));
                    }
                }
            }
            md.push('\n');
        }
    }

    if let Some(items) = output["action_items"].as_array() {
        if !items.is_empty() {
            md.push_str("## Action Items\n\n");
            for item in items {
                let desc = item["description"].as_str().unwrap_or("");
                let owner = item["owner"].as_str().unwrap_or("");
                let due = item["due_date"].as_str().unwrap_or("");
                let prio = item["priority"].as_str().unwrap_or("");
                md.push_str(&format!("- [ ] **{desc}**"));
                if !owner.is_empty() {
                    md.push_str(&format!(" — 👤 {owner}"));
                }
                if !due.is_empty() {
                    md.push_str(&format!(" 📅 {due}"));
                }
                if !prio.is_empty() {
                    md.push_str(&format!(" [{prio}]"));
                }
                md.push('\n');
            }
            md.push('\n');
        }
    }

    if let Some(draft) = output["follow_up_draft"].as_object() {
        if let Some(full_text) = draft.get("full_text").and_then(|v| v.as_str()) {
            if !full_text.is_empty() {
                md.push_str(&format!("## Follow-up E-Mail\n\n```\n{full_text}\n```\n\n"));
            }
        }
    }

    if let Some(parts) = output["participants_mentioned"].as_array() {
        if !parts.is_empty() {
            let names: Vec<&str> = parts
                .iter()
                .filter_map(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .collect();
            if !names.is_empty() {
                md.push_str("## Teilnehmer\n\n");
                md.push_str(&format!("{}\n\n", names.join(", ")));
            }
        }
    }

    md
}

/// Wraps a single line into multiple lines for PDF rendering.
/// Uses a single `Vec<char>` buffer with index tracking to avoid O(n²) re-allocation.
fn wrap_for_pdf(line: &str, max_chars: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }
    let max_chars = max_chars.max(8);
    let chars: Vec<char> = line.chars().collect();
    let total = chars.len();
    let mut out: Vec<String> = Vec::new();
    let mut start = 0;

    while start < total {
        let remaining = total - start;
        if remaining <= max_chars {
            out.push(chars[start..].iter().collect());
            break;
        }
        let end = start + max_chars;
        let slice = &chars[start..end];
        let mut cut = max_chars;
        if let Some(pos) = slice.iter().rposition(|&c| c == ' ') {
            if pos > max_chars / 4 {
                cut = pos + 1;
            }
        }
        let segment: String = chars[start..start + cut].iter().collect();
        out.push(segment.trim_end().to_string());
        start += cut;
        // Skip leading whitespace on the next line.
        while start < total && chars[start] == ' ' {
            start += 1;
        }
    }
    out
}

/// Renders Markdown as a simple multi-page A4 PDF (Helvetica).
pub fn generate_pdf(markdown: &str) -> Result<Vec<u8>, String> {
    let (doc, page1, layer1) = PdfDocument::new("Brief Export", Mm(210.0), Mm(297.0), "Layer 1");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| e.to_string())?;

    let mut page_idx = page1;
    let mut layer_idx = layer1;
    let mut y = 275.0_f32;
    let left = Mm(15.0);
    let bottom_margin = 18.0_f32;

    for line in markdown.lines() {
        let font_size = if line.starts_with("# ") {
            16.0
        } else if line.starts_with("## ") {
            13.0
        } else if line.starts_with("### ") {
            11.0
        } else {
            10.0
        };

        let text = line.trim_start_matches('#').trim();
        let display_lines = wrap_for_pdf(text, 95);

        for sub in display_lines {
            let line_height = font_size * 0.55 + 2.5;
            if y < bottom_margin + line_height {
                let (np, nl) = doc.add_page(Mm(210.0), Mm(297.0), "Layer 1");
                page_idx = np;
                layer_idx = nl;
                y = 275.0;
            }
            let current_layer = doc.get_page(page_idx).get_layer(layer_idx);
            let draw = if sub.is_empty() { " " } else { sub.as_str() };
            current_layer.use_text(draw, font_size, left, Mm(y), &font);
            y -= line_height;
        }
    }

    doc.save_to_bytes().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Builds a minimal meeting JSON for export tests.
    fn sample_meeting() -> serde_json::Value {
        json!({
            "title": "Quarterly Review",
            "created_at": "2025-03-01T10:00:00Z",
            "meeting_type": "consulting",
            "output": {
                "summary_short": "Revenue up 20%.",
                "topics": [
                    { "title": "Sales", "summary": "Q1 targets exceeded." },
                    { "title": "Hiring", "summary": "Three new engineers starting." }
                ],
                "decisions": [
                    { "description": "Budget increase approved", "context": "Pending CFO sign-off" }
                ],
                "action_items": [
                    {
                        "description": "Send updated forecast",
                        "owner": "Alice",
                        "due_date": "2025-03-15",
                        "priority": "high"
                    },
                    {
                        "description": "Schedule follow-up",
                        "owner": null,
                        "due_date": null,
                        "priority": null
                    }
                ],
                "follow_up_draft": {
                    "full_text": "Hi team,\nPlease find the notes attached.\nBest, Bob"
                },
                "participants_mentioned": ["Alice", "Bob"]
            }
        })
    }

    #[test]
    fn markdown_contains_title_and_date() {
        let md = generate_markdown(&sample_meeting());
        assert!(md.starts_with("# Quarterly Review"));
        assert!(md.contains("2025-03-01T10:00:00Z"));
        assert!(md.contains("consulting"));
    }

    #[test]
    fn markdown_contains_summary() {
        let md = generate_markdown(&sample_meeting());
        assert!(md.contains("## Zusammenfassung"));
        assert!(md.contains("Revenue up 20%"));
    }

    #[test]
    fn markdown_contains_topics() {
        let md = generate_markdown(&sample_meeting());
        assert!(md.contains("## Besprochene Themen"));
        assert!(md.contains("### Sales"));
        assert!(md.contains("Q1 targets exceeded."));
        assert!(md.contains("### Hiring"));
    }

    #[test]
    fn markdown_contains_decisions_with_context() {
        let md = generate_markdown(&sample_meeting());
        assert!(md.contains("## Entscheidungen"));
        assert!(md.contains("- Budget increase approved"));
        assert!(md.contains("Pending CFO sign-off"));
    }

    #[test]
    fn markdown_contains_action_items_with_metadata() {
        let md = generate_markdown(&sample_meeting());
        assert!(md.contains("## Action Items"));
        assert!(md.contains("**Send updated forecast**"));
        assert!(md.contains("👤 Alice"));
        assert!(md.contains("📅 2025-03-15"));
        assert!(md.contains("[high]"));
        // Second item with null fields should still appear but without metadata.
        assert!(md.contains("**Schedule follow-up**"));
    }

    #[test]
    fn markdown_contains_follow_up_email() {
        let md = generate_markdown(&sample_meeting());
        assert!(md.contains("## Follow-up E-Mail"));
        assert!(md.contains("Hi team,"));
    }

    #[test]
    fn markdown_contains_participants() {
        let md = generate_markdown(&sample_meeting());
        assert!(md.contains("## Teilnehmer"));
        assert!(md.contains("Alice, Bob"));
    }

    #[test]
    fn markdown_handles_empty_output_gracefully() {
        let meeting = json!({
            "title": "Empty",
            "created_at": "",
            "meeting_type": "",
            "output": {
                "summary_short": "",
                "topics": [],
                "decisions": [],
                "action_items": [],
                "follow_up_draft": {},
                "participants_mentioned": []
            }
        });
        let md = generate_markdown(&meeting);
        assert!(md.starts_with("# Empty"));
        // Should NOT contain section headers for empty arrays.
        assert!(!md.contains("## Besprochene Themen"));
        assert!(!md.contains("## Entscheidungen"));
        assert!(!md.contains("## Action Items"));
        assert!(!md.contains("## Zusammenfassung"));
        assert!(!md.contains("## Teilnehmer"));
    }

    #[test]
    fn markdown_handles_missing_keys() {
        // Simulate a minimal/broken meeting JSON — export should not panic.
        let meeting = json!({});
        let md = generate_markdown(&meeting);
        assert!(md.contains("# Meeting")); // fallback title
    }

    #[test]
    fn wrap_for_pdf_short_line_unchanged() {
        let lines = wrap_for_pdf("Hello world", 95);
        assert_eq!(lines, vec!["Hello world"]);
    }

    #[test]
    fn wrap_for_pdf_empty_line() {
        let lines = wrap_for_pdf("", 95);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn wrap_for_pdf_breaks_long_line_at_space() {
        let input = "This is a line that needs to be wrapped because it exceeds the limit";
        let lines = wrap_for_pdf(input, 30);
        assert!(lines.len() > 1);
        // No wrapped line should exceed the limit.
        for line in &lines {
            assert!(line.chars().count() <= 30, "Line too long: {line}");
        }
        // Recombined text should preserve all words.
        let joined = lines.join(" ");
        for word in input.split_whitespace() {
            assert!(joined.contains(word), "Missing word: {word}");
        }
    }

    #[test]
    fn wrap_for_pdf_min_width_clamped() {
        // Even with very small max_chars, the function should not panic.
        let lines = wrap_for_pdf("Short", 2);
        assert!(!lines.is_empty());
    }

    #[test]
    fn generate_pdf_produces_valid_bytes() {
        let md = generate_markdown(&sample_meeting());
        let bytes = generate_pdf(&md).expect("PDF generation should succeed");
        // PDF files always start with %PDF.
        assert!(bytes.len() > 100, "PDF too small");
        assert_eq!(&bytes[..5], b"%PDF-");
    }

    #[test]
    fn generate_pdf_empty_markdown() {
        let bytes = generate_pdf("").expect("Empty PDF should still succeed");
        assert!(bytes.len() > 50);
        assert_eq!(&bytes[..5], b"%PDF-");
    }

    // -- Additional edge cases --

    #[test]
    fn markdown_with_special_markdown_chars_in_fields() {
        let meeting = json!({
            "title": "Meeting with *bold* and `code`",
            "created_at": "2025-01-01T00:00:00Z",
            "meeting_type": "consulting",
            "output": {
                "summary_short": "Summary with [link](url) and **bold**",
                "topics": [{ "title": "Topic with # heading", "summary": "Details with > quote" }],
                "decisions": [{ "description": "Decision with `backticks`" }],
                "action_items": [],
                "follow_up_draft": {},
                "participants_mentioned": []
            }
        });
        let md = generate_markdown(&meeting);
        // Should not crash and should preserve the raw content.
        assert!(md.contains("*bold*"));
        assert!(md.contains("`code`"));
        assert!(md.contains("# heading"));
    }

    #[test]
    fn markdown_with_unicode_emoji_in_fields() {
        let meeting = json!({
            "title": "Sprint 🚀",
            "created_at": "2025-01-01T00:00:00Z",
            "meeting_type": "internal",
            "output": {
                "summary_short": "Team morale is high 💪",
                "topics": [],
                "decisions": [],
                "action_items": [{ "description": "Celebrate 🎉", "owner": "Everyone", "due_date": null, "priority": null }],
                "follow_up_draft": {},
                "participants_mentioned": ["Ünsal", "José"]
            }
        });
        let md = generate_markdown(&meeting);
        assert!(md.contains("Sprint 🚀"));
        assert!(md.contains("💪"));
        assert!(md.contains("🎉"));
        assert!(md.contains("Ünsal"));
    }

    #[test]
    fn wrap_for_pdf_word_longer_than_max_chars() {
        let long_word = "a".repeat(50);
        let lines = wrap_for_pdf(&long_word, 20);
        // Must not loop infinitely; should produce at least one line.
        assert!(!lines.is_empty());
        // The first line should contain at most max_chars.
        assert!(lines[0].chars().count() <= 20);
    }

    #[test]
    fn wrap_for_pdf_multiple_spaces() {
        let input = "word1    word2    word3";
        let lines = wrap_for_pdf(input, 95);
        // Short enough to fit in one line.
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn wrap_for_pdf_unicode_line() {
        let input = "Ärztlicher Beratungsgespräch über Überweisung zum Facharzt für Hals-Nasen-Ohren-Heilkunde";
        let lines = wrap_for_pdf(input, 40);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.chars().count() <= 40, "Line too long: {line}");
        }
    }

    #[test]
    fn generate_pdf_with_headings() {
        let md = "# Title\n\n## Section\n\nSome text.\n\n### Subsection\n\nMore text.";
        let bytes = generate_pdf(md).expect("PDF with headings should work");
        assert_eq!(&bytes[..5], b"%PDF-");
    }

    #[test]
    fn generate_pdf_with_long_content() {
        // Simulate ~5 pages of text.
        let paragraph = "This is a paragraph of text for testing. ".repeat(50);
        let md = format!("# Long Document\n\n{}", paragraph.repeat(5));
        let bytes = generate_pdf(&md).expect("Long PDF should succeed");
        assert!(bytes.len() > 1000);
    }
}
