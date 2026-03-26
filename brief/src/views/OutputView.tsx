import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { CSSProperties } from "react";
import { useEffect, useState } from "react";
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

interface OutputViewProps {
  /** Meeting to display (summary, topics, transcript, optional retained audio). */
  meeting: Meeting;
  /** Navigate back to history or home without persisting. */
  onBack: () => void;
}

/**
 * Read-only meeting detail: audio playback (if stored), AI output sections, Markdown/PDF/audio export via Tauri + system dialogs.
 *
 * @param props.meeting — loaded meeting record.
 * @param props.onBack — header back action.
 */
export function OutputView({ meeting, onBack }: OutputViewProps) {
  const { t } = useTranslation();
  const { exportBusy, exportError, exportMarkdown, exportPdf, exportAudio } = useExport();
  const [copied, setCopied] = useState(false);
  const [audioUrl, setAudioUrl] = useState<string | null>(null);

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

      <section className="output-section">
        <details>
          <summary style={{ cursor: "pointer", userSelect: "none", fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--color-text-subtle)", fontWeight: 600 }}>
            {t("output.transcript")}
          </summary>
          <pre className="transcript" style={{ marginTop: "0.5rem", fontFamily: "inherit" }}>
            {meeting.transcript}
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
