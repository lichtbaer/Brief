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

fn wrap_for_pdf(line: &str, max_chars: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }
    let max_chars = max_chars.max(8);
    let mut out: Vec<String> = Vec::new();
    let mut rest = line.to_string();
    while !rest.is_empty() {
        let count = rest.chars().count();
        if count <= max_chars {
            out.push(rest);
            break;
        }
        let chars: Vec<char> = rest.chars().collect();
        let mut cut = max_chars;
        let slice: &[char] = &chars[..max_chars];
        if let Some(pos) = slice.iter().rposition(|&c| c == ' ') {
            if pos > max_chars / 4 {
                cut = pos + 1;
            }
        }
        let left: String = chars[..cut].iter().collect();
        let right: String = chars[cut..].iter().collect();
        out.push(left.trim_end().to_string());
        rest = right.trim_start().to_string();
        if rest.is_empty() {
            break;
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
