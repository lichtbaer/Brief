import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { CSSProperties } from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useExport } from "../hooks/useExport";
import type {
  ActionItem,
  Decision,
  Meeting,
  Topic,
} from "../types";

export function safeExportBaseName(title: string): string {
  const trimmed = title.replace(/[/\\?%*:|"<>]/g, "-").trim();
  return trimmed.length > 0 ? trimmed : "meeting";
}

const PRIORITY_BADGE_STYLE: Record<
  NonNullable<ActionItem["priority"]>,
  CSSProperties
> = {
  high: { backgroundColor: "#fecaca", color: "#991b1b", borderColor: "#f87171" },
  medium: {
    backgroundColor: "#fef9c3",
    color: "#854d0e",
    borderColor: "#facc15",
  },
  low: { backgroundColor: "#dcfce7", color: "#166534", borderColor: "#4ade80" },
};

/** Extracts unique speaker label IDs from a raw transcript string (e.g. "SPEAKER_00", "SPEAKER_01"). */
function extractSpeakers(transcript: string): string[] {
  const matches = [...transcript.matchAll(/\[([A-Z_0-9]+)\]/g)];
  return [...new Set(matches.map((m) => m[1]))];
}

/**
 * Replaces speaker label brackets in the displayed transcript with user-defined names.
 * The original stored transcript is never modified — substitution is view-only.
 * Uses global regex replace for ES2020 compatibility (no String.prototype.replaceAll).
 */
function applyNames(transcript: string, names: Record<string, string>): string {
  return Object.entries(names).reduce((t, [id, name]) => {
    if (!name.trim()) return t;
    // Escape the label for use in a regex (brackets are special regex chars).
    const escaped = id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return t.replace(new RegExp(`\\[${escaped}\\]`, "g"), `[${name.trim()}]`);
  }, transcript);
}

interface OutputViewProps {
  /** Meeting to display (summary, topics, transcript, optional retained audio). */
  meeting: Meeting;
  /** Navigate back to history or home without persisting. */
  onBack: () => void;
}

/**
 * Read-only meeting detail: audio playback (if stored), AI output sections, tags editor,
 * speaker naming panel, and Markdown/PDF/audio export via Tauri + system dialogs.
 *
 * @param props.meeting — loaded meeting record.
 * @param props.onBack — header back action.
 */
export function OutputView({ meeting, onBack }: OutputViewProps) {
  const { t } = useTranslation();
  const { exportBusy, exportError, exportMarkdown, exportPdf, exportAudio } = useExport();
  const [copied, setCopied] = useState(false);
  const [audioUrl, setAudioUrl] = useState<string | null>(null);

  // Tags state — initialised from the loaded meeting, then kept in sync locally after mutations.
  const [tags, setTags] = useState<string[]>(meeting.tags ?? []);
  const [tagInput, setTagInput] = useState("");

  // Speaker name mapping state — keyed by speaker label (e.g. "SPEAKER_00").
  const [speakerNames, setSpeakerNames] = useState<Record<string, string>>(
    meeting.speaker_names ?? {}
  );
  const speakers = extractSpeakers(meeting.transcript);

  // Ref holding the latest names to avoid stale closure in the blur handler.
  const speakerNamesRef = useRef(speakerNames);
  speakerNamesRef.current = speakerNames;

  // Segment-level playback (per speaker) is planned for v1.1 — depends on diarization (BRIEF-SPIKE-001 / ADR-010).
  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      if (!meeting.audio_path) {
        setAudioUrl(null);
        return;
      }
      try {
        const path = await invoke<string>("get_audio_path", { id: meeting.id });
        if (!cancelled) {
          setAudioUrl(convertFileSrc(path));
        }
      } catch {
        if (!cancelled) {
          setAudioUrl(null);
        }
      }
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [meeting.id, meeting.audio_path]);

  const output = meeting.output;
  const followUp = output.follow_up_draft;
  const followUpText =
    followUp && typeof followUp.full_text === "string"
      ? followUp.full_text.trim()
      : "";

  const copyEmail = () => {
    void navigator.clipboard.writeText(followUpText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  /** Adds a new tag (commit on Enter or comma) and persists to backend. */
  const commitTag = async (raw: string) => {
    const tag = raw.replace(/,/g, "").trim();
    if (!tag || tags.includes(tag) || tag.length > 50) return;
    const updated = [...tags, tag];
    setTags(updated);
    setTagInput("");
    try {
      await invoke("update_meeting_tags", { id: meeting.id, tags: updated });
    } catch {
      // Revert optimistic update on failure.
      setTags(tags);
    }
  };

  /** Removes an existing tag and persists to backend. */
  const removeTag = async (tag: string) => {
    const updated = tags.filter((t) => t !== tag);
    setTags(updated);
    try {
      await invoke("update_meeting_tags", { id: meeting.id, tags: updated });
    } catch {
      setTags(tags);
    }
  };

  /** Persists the full speaker names map after a single field loses focus. */
  const persistSpeakerNames = async (names: Record<string, string>) => {
    try {
      await invoke("update_speaker_names", { id: meeting.id, names });
    } catch {
      // Failure is non-critical — names are cosmetic and will be re-entered on next open.
    }
  };

  return (
    <div className="output-view" style={{ maxWidth: "52rem" }}>
      <div className="output-header" style={{ marginBottom: "1.5rem" }}>
        <button type="button" className="btn btn-ghost btn-icon" onClick={onBack}>
          ← {t("output.back")}
        </button>
        <h1 style={{ marginTop: "0.75rem", marginBottom: "0.25rem", fontSize: "1.3rem", fontWeight: 700 }}>
          {meeting.title}
        </h1>
        <span style={{ fontSize: "0.85rem", color: "var(--color-text-muted)" }}>
          {t(`meeting_types.${meeting.meeting_type}`)}
        </span>

        {exportError && (
          <div className="alert alert-error" style={{ marginTop: "0.75rem" }}>
            <span>⚠</span>
            <span>{t("errors.alert", { message: exportError })}</span>
          </div>
        )}

        <div
          className="export-buttons"
          style={{ display: "flex", flexWrap: "wrap", gap: "0.5rem", marginTop: "1rem" }}
        >
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            disabled={exportBusy !== null}
            onClick={() => { void exportMarkdown(meeting.id, meeting.title); }}
          >
            {exportBusy === "markdown" ? (
              <><span className="spinner spinner-dark" />{t("output.exporting")}</>
            ) : (
              t("output.export_markdown")
            )}
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            disabled={exportBusy !== null}
            onClick={() => { void exportPdf(meeting.id, meeting.title); }}
          >
            {exportBusy === "pdf" ? (
              <><span className="spinner spinner-dark" />{t("output.exporting")}</>
            ) : (
              t("output.export_pdf")
            )}
          </button>
          {meeting.audio_path ? (
            <button
              type="button"
              className="btn btn-ghost btn-icon"
              disabled={exportBusy !== null}
              onClick={() => { void exportAudio(meeting.id); }}
            >
              {exportBusy === "audio" ? (
                <><span className="spinner spinner-dark" />{t("output.exporting")}</>
              ) : (
                t("output.export_audio")
              )}
            </button>
          ) : null}
        </div>
      </div>

      {/* Tags section */}
      <section className="output-section">
        <h2>{t("output.tags_title")}</h2>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "0.4rem", marginTop: "0.5rem", alignItems: "center" }}>
          {tags.map((tag) => (
            <span
              key={tag}
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: "0.25rem",
                fontSize: "0.8rem",
                padding: "0.15rem 0.55rem",
                borderRadius: "999px",
                border: "1px solid var(--color-border, #d1d5db)",
                color: "var(--color-text-muted)",
              }}
            >
              {tag}
              <button
                type="button"
                onClick={() => void removeTag(tag)}
                aria-label={`Remove tag ${tag}`}
                style={{
                  background: "none",
                  border: "none",
                  cursor: "pointer",
                  padding: 0,
                  lineHeight: 1,
                  color: "var(--color-text-subtle)",
                  fontSize: "0.9rem",
                }}
              >
                ×
              </button>
            </span>
          ))}
          <input
            type="text"
            value={tagInput}
            placeholder={t("output.tags_add_placeholder")}
            onChange={(e) => {
              const val = e.target.value;
              // Commit immediately when the user types a comma.
              if (val.includes(",")) {
                void commitTag(val);
              } else {
                setTagInput(val);
              }
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                void commitTag(tagInput);
              }
            }}
            style={{
              fontSize: "0.8rem",
              border: "1px solid var(--color-border, #d1d5db)",
              borderRadius: "999px",
              padding: "0.15rem 0.6rem",
              outline: "none",
              background: "transparent",
              color: "var(--color-text)",
              minWidth: "8rem",
            }}
          />
        </div>
      </section>

      <section className="output-section">
        <h2>{t("output.audio_recording")}</h2>
        {audioUrl ? (
          <audio controls src={audioUrl} aria-label={t("output.audio_recording")} style={{ width: "100%", marginTop: "0.5rem" }} />
        ) : (
          <p style={{ marginTop: "0.5rem", color: "var(--color-text-muted)", fontSize: "0.9rem" }}>
            {t("output.audio_not_saved")}
          </p>
        )}
      </section>

      <section className="output-section">
        <h2>{t("output.summary")}</h2>
        <p style={{ marginTop: "0.5rem", whiteSpace: "pre-wrap", lineHeight: 1.6 }}>
          {output.summary_short}
        </p>
      </section>

      {output.topics.length > 0 && (
        <section className="output-section">
          <h2>{t("output.topics")}</h2>
          {output.topics.map((topic: Topic, i: number) => (
            <div key={i} style={{ marginTop: "0.75rem" }}>
              <h3 style={{ fontSize: "0.95rem", fontWeight: 600, marginBottom: "0.25rem" }}>
                {topic.title}
              </h3>
              <p style={{ margin: 0, whiteSpace: "pre-wrap", color: "var(--color-text-muted)", lineHeight: 1.55 }}>
                {topic.summary}
              </p>
            </div>
          ))}
        </section>
      )}

      {output.decisions.length > 0 && (
        <section className="output-section">
          <h2>{t("output.decisions")}</h2>
          <ul style={{ paddingLeft: "1.25rem", marginTop: "0.5rem" }}>
            {output.decisions.map((d: Decision, i: number) => (
              <li key={i} style={{ marginBottom: "0.5rem", lineHeight: 1.55 }}>
                <strong>{d.description}</strong>
                {d.context && (
                  <p style={{ margin: "0.25rem 0 0", color: "var(--color-text-muted)", fontSize: "0.9rem" }}>
                    {d.context}
                  </p>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}

      {output.action_items.length > 0 && (
        <section className="output-section">
          <h2>{t("output.action_items")}</h2>
          {output.action_items.map((item: ActionItem, i: number) => (
            <div key={i} className="action-item" style={{ marginTop: "0.75rem" }}>
              <p style={{ margin: "0 0 0.5rem", whiteSpace: "pre-wrap", lineHeight: 1.5 }}>
                {item.description}
              </p>
              <div style={{ display: "flex", flexWrap: "wrap", gap: "0.75rem", fontSize: "0.85rem", color: "var(--color-text-muted)" }}>
                {item.owner && (
                  <span>{t("output.action_owner", { owner: item.owner })}</span>
                )}
                {item.due_date && (
                  <span>{t("output.action_due", { date: item.due_date })}</span>
                )}
                {item.priority && (
                  <span
                    className={`priority-badge ${item.priority}`}
                    style={{
                      padding: "0.1rem 0.5rem",
                      borderRadius: "var(--radius-sm)",
                      border: "1px solid",
                      fontWeight: 600,
                      ...PRIORITY_BADGE_STYLE[item.priority],
                    }}
                  >
                    {item.priority}
                  </span>
                )}
              </div>
            </div>
          ))}
        </section>
      )}

      {followUpText.length > 0 && (
        <section className="output-section">
          <h2>{t("output.follow_up_draft")}</h2>
          <div style={{ marginTop: "0.5rem" }}>
            {followUp.subject && (
              <p style={{ marginBottom: "0.5rem" }}>
                <strong>{t("output.subject")}:</strong> {followUp.subject}
              </p>
            )}
            <pre className="email-body" style={{ fontFamily: "inherit" }}>
              {followUpText}
            </pre>
          </div>
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            style={{ marginTop: "0.75rem" }}
            onClick={copyEmail}
          >
            {copied ? t("output.copied") : t("output.copy_email")}
          </button>
        </section>
      )}

      {/* Speaker naming panel — only shown when the transcript contains diarized speaker labels */}
      {speakers.length > 0 && (
        <section className="output-section">
          <h2>{t("output.speaker_names_title")}</h2>
          <p style={{ fontSize: "0.8rem", color: "var(--color-text-muted)", marginTop: "0.25rem", marginBottom: "0.75rem" }}>
            {t("output.speaker_names_hint")}
          </p>
          <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
            {speakers.map((id) => (
              <div key={id} style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
                <span
                  style={{
                    fontSize: "0.8rem",
                    fontFamily: "monospace",
                    color: "var(--color-text-muted)",
                    minWidth: "8rem",
                  }}
                >
                  {id}
                </span>
                <input
                  type="text"
                  value={speakerNames[id] ?? ""}
                  placeholder={t("output.speaker_name_placeholder")}
                  onChange={(e) => {
                    // Update local state immediately for responsive UX.
                    const updated = { ...speakerNamesRef.current, [id]: e.target.value };
                    setSpeakerNames(updated);
                  }}
                  onBlur={() => {
                    // Persist the full map once the user leaves the input field.
                    void persistSpeakerNames(speakerNamesRef.current);
                  }}
                  className="form-input"
                  style={{ maxWidth: "16rem", fontSize: "0.875rem" }}
                />
              </div>
            ))}
          </div>
        </section>
      )}

      <section className="output-section">
        <details>
          <summary style={{ cursor: "pointer", userSelect: "none", fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--color-text-subtle)", fontWeight: 600 }}>
            {t("output.transcript")}
          </summary>
          {/* Apply speaker name substitutions at render time only.
              The stored transcript preserves original labels for FTS indexing. */}
          <pre className="transcript" style={{ marginTop: "0.5rem", fontFamily: "inherit" }}>
            {applyNames(meeting.transcript, speakerNames)}
          </pre>
        </details>
      </section>

      {output.participants_mentioned.length > 0 && (
        <section className="output-section">
          <h2>{t("output.participants")}</h2>
          <p style={{ marginTop: "0.25rem" }}>
            {output.participants_mentioned.join(", ")}
          </p>
        </section>
      )}
    </div>
  );
}
