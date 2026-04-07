import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { CSSProperties, ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useExport } from "../hooks/useExport";
import { formatDuration } from "./HistoryView";
import type {
  ActionItem,
  Decision,
  Meeting,
  SettingDefaults,
  Topic,
} from "../types";
// Re-export so callers that imported safeExportBaseName from here continue to work.
export { safeExportBaseName } from "../utils/exportUtils";

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

/**
 * Splits `text` on case-insensitive occurrences of `query` and returns an array of React nodes
 * where matched portions are wrapped in `<mark>` elements. Used for transcript search highlighting.
 * Returns plain text when `query` is empty to avoid unnecessary React reconciliation.
 */
function highlightTerms(text: string, query: string): ReactNode {
  if (!query.trim()) return text;
  // Truncate before escaping to prevent ReDoS: a very long query with special characters
  // can create a regex that causes catastrophic backtracking on large transcripts.
  const safeQuery = query.slice(0, 200);
  // Build a case-insensitive regex from the query; escape special regex characters.
  const escaped = safeQuery.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const parts = text.split(new RegExp(`(${escaped})`, "gi"));
  return parts.map((part, i) =>
    // Every odd index is a match (the captured group from the regex split).
    i % 2 === 1 ? <mark key={i}>{part}</mark> : part
  );
}

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
  /** Called with the refreshed meeting after a successful re-summarization. */
  onMeetingUpdated?: (updated: Meeting) => void;
  /** Optional search query that opened this meeting — used to highlight matching terms in the transcript. */
  searchQuery?: string;
  /** Called when the user clicks a participant name to filter history by that person. */
  onFilterByParticipant?: (name: string) => void;
}

/**
 * Read-only meeting detail: audio playback (if stored), AI output sections, tags editor,
 * speaker naming panel, and Markdown/PDF/audio export via Tauri + system dialogs.
 *
 * @param props.meeting — loaded meeting record.
 * @param props.onBack — header back action.
 * @param props.searchQuery — optional search term; highlights matching text in the transcript.
 * @param props.onFilterByParticipant — callback for participant name clicks; filters history view.
 */
export function OutputView({ meeting, onBack, onMeetingUpdated, searchQuery, onFilterByParticipant }: OutputViewProps) {
  const { t } = useTranslation();
  const { exportBusy, exportError, exportSuccess, exportMarkdown, exportPdf, exportAudio, exportCsv } = useExport();
  const [copied, setCopied] = useState(false);
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [regenerating, setRegenerating] = useState(false);
  // Shown when speaker name persistence fails — cosmetic, but user should know the save failed.
  const [speakerSaveError, setSpeakerSaveError] = useState(false);

  // Ref to the <audio> element for click-to-seek segment playback (Feature 2).
  const audioRef = useRef<HTMLAudioElement>(null);
  // Index of the segment currently being played — updated via the timeupdate event.
  const [currentSegmentIndex, setCurrentSegmentIndex] = useState<number | null>(null);

  // Stale analysis warning: dismissed by the user per session (Feature 4).
  const [staleWarningDismissed, setStaleWarningDismissed] = useState(false);
  const [currentTemplateVersion, setCurrentTemplateVersion] = useState<string | null>(null);

  // Follow-up email draft editing state.
  const [followUpEditText, setFollowUpEditText] = useState<string | null>(null);
  const [followUpSaveError, setFollowUpSaveError] = useState(false);

  // Title editing state — local copy so edits are reflected immediately before persistence.
  const [title, setTitle] = useState(meeting.title);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleSaveError, setTitleSaveError] = useState(false);

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

  // Loads the retained audio URL from the backend; resets when meeting changes.
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

  // Fetch the current template version from the backend once per mount to power the
  // stale-analysis warning banner (Feature 4).
  useEffect(() => {
    void invoke<SettingDefaults>("get_setting_defaults")
      .then((d) => setCurrentTemplateVersion(d.template_version))
      .catch(() => { /* non-fatal */ });
  }, []);

  // Tracks which transcript segment is currently audible so the active row can be highlighted
  // and the seek buttons can reflect playback state (Feature 2).
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !meeting.segments || meeting.segments.length === 0) return;
    const segments = meeting.segments;

    const onTimeUpdate = () => {
      const t = audio.currentTime;
      // Find the last segment whose start time is ≤ current playback position.
      let active = -1;
      for (let i = 0; i < segments.length; i++) {
        if (segments[i].start <= t) active = i;
        else break;
      }
      setCurrentSegmentIndex(active >= 0 ? active : null);
    };

    audio.addEventListener("timeupdate", onTimeUpdate);
    return () => audio.removeEventListener("timeupdate", onTimeUpdate);
  }, [meeting.segments, audioUrl]);

  const output = meeting.output;
  const followUp = output.follow_up_draft;
  const followUpText =
    followUp && typeof followUp.full_text === "string"
      ? followUp.full_text.trim()
      : "";

  /** Persists the edited title to the backend; reverts to the previous value on failure. */
  const persistTitle = async (newTitle: string) => {
    const trimmed = newTitle.trim();
    if (!trimmed || trimmed === meeting.title) {
      setTitle(meeting.title);
      setEditingTitle(false);
      return;
    }
    try {
      await invoke("update_meeting_title", { id: meeting.id, title: trimmed });
      setTitle(trimmed);
      setTitleSaveError(false);
    } catch {
      setTitle(meeting.title);
      setTitleSaveError(true);
      setTimeout(() => setTitleSaveError(false), 5000);
    }
    setEditingTitle(false);
  };

  /** Prompts for confirmation then soft-deletes the meeting and navigates back. */
  const deleteMeeting = async () => {
    if (!window.confirm(t("output.delete_confirm"))) return;
    setDeleting(true);
    try {
      await invoke("delete_meeting", { id: meeting.id });
      onBack();
    } catch {
      setDeleting(false);
      alert(t("output.delete_error"));
    }
  };

  /** Re-runs the Ollama summarizer on the stored transcript; updates the view with the new output. */
  const regenerateSummary = async () => {
    setRegenerating(true);
    try {
      const json = await invoke<string>("regenerate_summary", {
        id: meeting.id,
        meetingType: meeting.meeting_type,
      });
      const updated = JSON.parse(json) as Meeting;
      if (onMeetingUpdated) onMeetingUpdated(updated);
    } catch {
      alert(t("output.regenerate_error"));
    } finally {
      setRegenerating(false);
    }
  };

  const copyEmail = () => {
    // Copy the edited text if editing, otherwise the persisted text.
    void navigator.clipboard.writeText(followUpEditText ?? followUpText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  /** Persists the edited follow-up draft to the backend. */
  const saveFollowUpDraft = async () => {
    if (followUpEditText === null) return;
    try {
      await invoke("update_follow_up_draft", { id: meeting.id, text: followUpEditText });
      setFollowUpEditText(null);
      setFollowUpSaveError(false);
    } catch {
      setFollowUpSaveError(true);
      setTimeout(() => setFollowUpSaveError(false), 5000);
    }
  };

  // Maximum tag length and count limits — kept in sync with backend validation in storage.rs.
  const TAG_MAX_LENGTH = 50;

  /** Adds a new tag (commit on Enter or comma) and persists to backend. */
  const commitTag = async (raw: string) => {
    const tag = raw.replace(/,/g, "").trim();
    // Case-insensitive duplicate check: "Projekt" and "projekt" are treated as the same tag.
    const isDuplicate = tags.some((t) => t.toLowerCase() === tag.toLowerCase());
    if (!tag || isDuplicate || tag.length > TAG_MAX_LENGTH) return;
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
      setSpeakerSaveError(false);
    } catch {
      // Show a brief inline error so the user knows the name was not saved.
      setSpeakerSaveError(true);
      setTimeout(() => setSpeakerSaveError(false), 5000);
    }
  };

  return (
    <div className="output-view" style={{ maxWidth: "52rem" }}>
      <div className="output-header" style={{ marginBottom: "1.5rem" }}>
        <button type="button" className="btn btn-ghost btn-icon" onClick={onBack}>
          ← {t("output.back")}
        </button>
        {/* Inline-editable meeting title — click to edit, blur/Enter to persist */}
        {editingTitle ? (
          <input
            type="text"
            className="form-input"
            value={title}
            placeholder={t("output.title_edit_placeholder")}
            onChange={(e) => setTitle(e.target.value)}
            onBlur={() => void persistTitle(title)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void persistTitle(title);
              if (e.key === "Escape") { setTitle(meeting.title); setEditingTitle(false); }
            }}
            autoFocus
            style={{ marginTop: "0.75rem", marginBottom: "0.25rem", fontSize: "1.3rem", fontWeight: 700, maxWidth: "100%" }}
          />
        ) : (
          <h1
            style={{ marginTop: "0.75rem", marginBottom: "0.25rem", fontSize: "1.3rem", fontWeight: 700, cursor: "text" }}
            title={t("output.title_edit_placeholder")}
            onClick={() => setEditingTitle(true)}
          >
            {title}
          </h1>
        )}
        {titleSaveError && (
          <div className="alert alert-error" role="alert" style={{ marginBottom: "0.5rem", fontSize: "0.85rem" }}>
            <span>⚠</span>
            <span>{t("output.title_save_error")}</span>
          </div>
        )}
        <span style={{ fontSize: "0.85rem", color: "var(--color-text-muted)", display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
          <span>{t(`meeting_types.${meeting.meeting_type}`)}</span>
          {meeting.duration_seconds > 0 && (
            <span>{formatDuration(meeting.duration_seconds)}</span>
          )}
        </span>

        {exportError && (
          <div className="alert alert-error" style={{ marginTop: "0.75rem" }}>
            <span>⚠</span>
            <span>{t("errors.alert", { message: exportError })}</span>
          </div>
        )}

        {exportSuccess && (
          <div className="alert alert-success" style={{ marginTop: "0.75rem" }}>
            <span>✓</span>
            <span>{t("output.export_audio_success", { path: exportSuccess })}</span>
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
            onClick={() => { void exportMarkdown(meeting.id, title); }}
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
            onClick={() => { void exportPdf(meeting.id, title); }}
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
          {meeting.output.action_items.length > 0 && (
            <button
              type="button"
              className="btn btn-ghost btn-icon"
              disabled={exportBusy !== null}
              onClick={() => { void exportCsv(meeting.id); }}
            >
              {exportBusy === "csv" ? (
                <><span className="spinner spinner-dark" />{t("output.exporting")}</>
              ) : (
                t("output.export_csv")
              )}
            </button>
          )}
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            disabled={regenerating || deleting || exportBusy !== null}
            onClick={() => void regenerateSummary()}
          >
            {regenerating ? (
              <><span className="spinner spinner-dark" />{t("output.regenerating")}</>
            ) : (
              t("output.regenerate_summary")
            )}
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            disabled={deleting || regenerating || exportBusy !== null}
            onClick={() => void deleteMeeting()}
            style={{ color: "var(--color-error, #dc2626)", marginLeft: "auto" }}
          >
            {deleting ? (
              <><span className="spinner spinner-dark" />{t("output.delete_meeting")}</>
            ) : (
              t("output.delete_meeting")
            )}
          </button>
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

      {/* Stale analysis warning — shown when stored template_version is older than current (Feature 4) */}
      {!staleWarningDismissed
        && currentTemplateVersion !== null
        && meeting.output.template_version
        && meeting.output.template_version !== currentTemplateVersion && (
        <div
          className="alert"
          role="status"
          style={{
            display: "flex",
            alignItems: "flex-start",
            gap: "0.75rem",
            justifyContent: "space-between",
            marginBottom: "1rem",
            background: "var(--color-warning-bg, #fef9c3)",
            border: "1px solid var(--color-warning, #facc15)",
            borderRadius: "var(--radius)",
            padding: "0.75rem 1rem",
            fontSize: "0.875rem",
          }}
        >
          <span>{t("output.stale_analysis_warning")}</span>
          <button
            type="button"
            onClick={() => setStaleWarningDismissed(true)}
            aria-label={t("output.stale_analysis_dismiss")}
            style={{
              background: "none",
              border: "none",
              cursor: "pointer",
              padding: 0,
              fontSize: "1rem",
              lineHeight: 1,
              color: "var(--color-text-muted)",
              flexShrink: 0,
            }}
          >
            ×
          </button>
        </div>
      )}

      <section className="output-section">
        <h2>{t("output.audio_recording")}</h2>
        {audioUrl ? (
          // ref is needed for click-to-seek segment playback (Feature 2).
          <audio ref={audioRef} controls src={audioUrl} aria-label={t("output.audio_recording")} style={{ width: "100%", marginTop: "0.5rem" }} />
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
            {followUp.subject && followUpEditText === null && (
              <p style={{ marginBottom: "0.5rem" }}>
                <strong>{t("output.subject")}:</strong> {followUp.subject}
              </p>
            )}
            {/* Editable textarea — shown while editing; read-only pre shown otherwise */}
            {followUpEditText !== null ? (
              <textarea
                className="form-input"
                value={followUpEditText}
                onChange={(e) => setFollowUpEditText(e.target.value)}
                style={{ width: "100%", minHeight: "12rem", fontFamily: "inherit", fontSize: "0.9rem", whiteSpace: "pre-wrap", resize: "vertical" }}
                aria-label={t("output.follow_up_draft")}
              />
            ) : (
              <pre className="email-body" style={{ fontFamily: "inherit" }}>
                {followUpText}
              </pre>
            )}
          </div>
          {followUpSaveError && (
            <div className="alert alert-error" role="alert" style={{ marginTop: "0.5rem", fontSize: "0.85rem" }}>
              <span>⚠</span>
              <span>{t("output.follow_up_save_error")}</span>
            </div>
          )}
          <div style={{ display: "flex", gap: "0.5rem", marginTop: "0.75rem", flexWrap: "wrap" }}>
            {followUpEditText === null ? (
              <>
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  onClick={() => setFollowUpEditText(followUpText)}
                >
                  {t("output.follow_up_edit")}
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  onClick={copyEmail}
                >
                  {copied ? t("output.copied") : t("output.copy_email")}
                </button>
              </>
            ) : (
              <>
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  onClick={() => void saveFollowUpDraft()}
                >
                  {t("output.follow_up_save")}
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  onClick={() => { setFollowUpEditText(null); setFollowUpSaveError(false); }}
                >
                  {t("output.follow_up_cancel")}
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-icon"
                  onClick={copyEmail}
                >
                  {copied ? t("output.copied") : t("output.copy_email")}
                </button>
              </>
            )}
          </div>
        </section>
      )}

      {/* Speaker naming panel — only shown when the transcript contains diarized speaker labels */}
      {speakers.length > 0 && (
        <section className="output-section">
          <h2>{t("output.speaker_names_title")}</h2>
          <p style={{ fontSize: "0.8rem", color: "var(--color-text-muted)", marginTop: "0.25rem", marginBottom: "0.75rem" }}>
            {t("output.speaker_names_hint")}
          </p>
          {speakerSaveError && (
            <div className="alert alert-error" role="alert" style={{ marginBottom: "0.75rem", fontSize: "0.85rem" }}>
              <span>⚠</span>
              <span>{t("output.speaker_names_save_error")}</span>
            </div>
          )}
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
                  maxLength={50}
                  onChange={(e) => {
                    // Enforce max 50 chars; trim leading/trailing whitespace on persist (blur handler).
                    const updated = { ...speakerNamesRef.current, [id]: e.target.value.slice(0, 50) };
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
        {/* Auto-open the details element when the user arrived via a search query (Feature 6) */}
        <details open={!!searchQuery}>
          <summary style={{ cursor: "pointer", userSelect: "none", fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--color-text-subtle)", fontWeight: 600 }}>
            {t("output.transcript")}
          </summary>
          {/* When diarized segments are available, render them with timestamps for click-to-seek.
              Fall back to the plain transcript for older meetings without segment data. */}
          {meeting.segments && meeting.segments.length > 0 ? (
            <div style={{ marginTop: "0.5rem", display: "flex", flexDirection: "column", gap: "0.4rem" }}>
              {meeting.segments.map((seg, i) => {
                const displaySpeaker = speakerNames[seg.speaker] || seg.speaker;
                const startSec = Math.floor(seg.start);
                const mins = String(Math.floor(startSec / 60)).padStart(2, "0");
                const secs = String(startSec % 60).padStart(2, "0");
                const isActive = currentSegmentIndex === i;
                return (
                  // Wrap each segment row in a button so clicking it seeks the audio player
                  // to the segment's start time (Feature 2). Disabled when no audio is retained.
                  <button
                    key={i}
                    type="button"
                    disabled={!audioUrl}
                    aria-label={t("output.seek_to_segment_aria", { time: `${mins}:${secs}` })}
                    onClick={() => {
                      const audio = audioRef.current;
                      if (!audio) return;
                      audio.currentTime = seg.start;
                      void audio.play();
                    }}
                    style={{
                      display: "flex",
                      gap: "0.75rem",
                      alignItems: "flex-start",
                      fontSize: "0.875rem",
                      lineHeight: 1.5,
                      background: isActive ? "var(--color-accent-subtle, rgba(59,130,246,0.08))" : "none",
                      border: "none",
                      borderLeft: isActive ? "3px solid var(--color-accent, #3b82f6)" : "3px solid transparent",
                      borderRadius: 0,
                      cursor: audioUrl ? "pointer" : "default",
                      padding: "0.15rem 0.25rem",
                      textAlign: "left",
                      width: "100%",
                    }}
                  >
                    <span style={{ fontFamily: "monospace", color: "var(--color-text-subtle)", minWidth: "3.5rem", flexShrink: 0, paddingTop: "0.05rem" }}>
                      {mins}:{secs}
                    </span>
                    <span style={{ fontFamily: "monospace", color: "var(--color-text-muted)", minWidth: "6.5rem", flexShrink: 0, paddingTop: "0.05rem" }}>
                      {displaySpeaker}
                    </span>
                    <span style={{ flex: 1 }}>
                      {/* Apply search term highlighting on segment text (Feature 6) */}
                      {searchQuery ? highlightTerms(seg.text, searchQuery) : seg.text}
                    </span>
                  </button>
                );
              })}
            </div>
          ) : (
            <pre className="transcript" style={{ marginTop: "0.5rem", fontFamily: "inherit" }}>
              {/* Apply search highlighting and speaker name substitution on plain transcript */}
              {searchQuery
                ? highlightTerms(applyNames(meeting.transcript, speakerNames), searchQuery)
                : applyNames(meeting.transcript, speakerNames)}
            </pre>
          )}
        </details>
      </section>

      {output.participants_mentioned.length > 0 && (
        <section className="output-section">
          <h2>{t("output.participants")}</h2>
          {/* Participant names are rendered as clickable buttons when a filter callback is provided
              so the user can jump to a history view filtered to all meetings with that person (Feature 7). */}
          {onFilterByParticipant ? (
            <>
              <p style={{ marginTop: "0.25rem", marginBottom: "0.35rem", fontSize: "0.8rem", color: "var(--color-text-muted)" }}>
                {t("output.participants_filter_hint")}
              </p>
              <div style={{ display: "flex", flexWrap: "wrap", gap: "0.4rem" }}>
                {output.participants_mentioned.map((name) => (
                  <button
                    key={name}
                    type="button"
                    onClick={() => onFilterByParticipant(name)}
                    style={{
                      fontSize: "0.875rem",
                      padding: "0.2rem 0.6rem",
                      borderRadius: "999px",
                      border: "1px solid var(--color-accent, #3b82f6)",
                      background: "transparent",
                      color: "var(--color-accent, #3b82f6)",
                      cursor: "pointer",
                    }}
                  >
                    {name}
                  </button>
                ))}
              </div>
            </>
          ) : (
            <p style={{ marginTop: "0.25rem" }}>
              {output.participants_mentioned.join(", ")}
            </p>
          )}
        </section>
      )}
    </div>
  );
}
