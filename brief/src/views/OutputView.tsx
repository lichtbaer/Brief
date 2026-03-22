import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import type { CSSProperties } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  ActionItem,
  Decision,
  Meeting,
  Topic,
} from "../types";

function safeExportBaseName(title: string): string {
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
  meeting: Meeting;
  onBack: () => void;
}

export function OutputView({ meeting, onBack }: OutputViewProps) {
  const { t } = useTranslation();
  const [exportBusy, setExportBusy] = useState<"markdown" | "pdf" | null>(null);
  const output = meeting.output;
  const followUp = output.follow_up_draft;
  const followUpText =
    followUp && typeof followUp.full_text === "string"
      ? followUp.full_text.trim()
      : "";

  const exportMarkdown = async () => {
    setExportBusy("markdown");
    try {
      const markdown = await invoke<string>("export_markdown", {
        id: meeting.id,
      });
      const base = safeExportBaseName(meeting.title);
      const path = await save({
        defaultPath: `${base}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (path) {
        await writeFile(path, new TextEncoder().encode(markdown));
      }
    } catch (e) {
      window.alert(t("errors.alert", { message: String(e) }));
    } finally {
      setExportBusy(null);
    }
  };

  const exportPdf = async () => {
    setExportBusy("pdf");
    try {
      const pdfBase64 = await invoke<string>("export_pdf", { id: meeting.id });
      const base = safeExportBaseName(meeting.title);
      const path = await save({
        defaultPath: `${base}.pdf`,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (path) {
        const bytes = Uint8Array.from(atob(pdfBase64), (c) => c.charCodeAt(0));
        await writeFile(path, bytes);
      }
    } catch (e) {
      window.alert(t("errors.alert", { message: String(e) }));
    } finally {
      setExportBusy(null);
    }
  };

  return (
    <div className="output-view" style={{ maxWidth: "52rem" }}>
      <div className="output-header" style={{ marginBottom: "1.5rem" }}>
        <button type="button" onClick={onBack}>
          {t("output.back")}
        </button>
        <h1 style={{ marginTop: "0.75rem", marginBottom: "0.25rem" }}>
          {meeting.title}
        </h1>
        <span className="meeting-type" style={{ color: "#525252" }}>
          {t(`meeting_types.${meeting.meeting_type}`)}
        </span>
        <div
          className="export-buttons"
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "0.5rem",
            marginTop: "1rem",
          }}
        >
          <button
            type="button"
            disabled={exportBusy !== null}
            onClick={() => {
              void exportMarkdown();
            }}
          >
            {t("output.export_markdown")}
          </button>
          <button
            type="button"
            disabled={exportBusy !== null}
            onClick={() => {
              void exportPdf();
            }}
          >
            {t("output.export_pdf")}
          </button>
        </div>
      </div>

      <section className="output-section" style={{ marginBottom: "1.5rem" }}>
        <h2 style={{ fontSize: "1.125rem" }}>{t("output.summary")}</h2>
        <p style={{ marginTop: "0.5rem", whiteSpace: "pre-wrap" }}>
          {output.summary_short}
        </p>
      </section>

      {output.topics.length > 0 && (
        <section className="output-section" style={{ marginBottom: "1.5rem" }}>
          <h2 style={{ fontSize: "1.125rem" }}>{t("output.topics")}</h2>
          {output.topics.map((topic: Topic, i: number) => (
            <div
              key={i}
              className="topic-item"
              style={{ marginTop: "0.75rem" }}
            >
              <h3 style={{ fontSize: "1rem", marginBottom: "0.25rem" }}>
                {topic.title}
              </h3>
              <p style={{ margin: 0, whiteSpace: "pre-wrap" }}>{topic.summary}</p>
            </div>
          ))}
        </section>
      )}

      {output.decisions.length > 0 && (
        <section className="output-section" style={{ marginBottom: "1.5rem" }}>
          <h2 style={{ fontSize: "1.125rem" }}>{t("output.decisions")}</h2>
          <ul style={{ paddingLeft: "1.25rem", marginTop: "0.5rem" }}>
            {output.decisions.map((d: Decision, i: number) => (
              <li key={i} style={{ marginBottom: "0.5rem" }}>
                <strong>{d.description}</strong>
                {d.context && (
                  <p className="context" style={{ margin: "0.25rem 0 0", color: "#525252" }}>
                    {d.context}
                  </p>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}

      {output.action_items.length > 0 && (
        <section className="output-section" style={{ marginBottom: "1.5rem" }}>
          <h2 style={{ fontSize: "1.125rem" }}>{t("output.action_items")}</h2>
          {output.action_items.map((item: ActionItem, i: number) => (
            <div
              key={i}
              className={`action-item priority-${item.priority ?? "none"}`}
              style={{
                marginTop: "0.75rem",
                padding: "0.75rem",
                border: "1px solid #e5e5e5",
                borderRadius: "6px",
              }}
            >
              <p style={{ margin: "0 0 0.5rem", whiteSpace: "pre-wrap" }}>
                {item.description}
              </p>
              <div
                className="action-meta"
                style={{ display: "flex", flexWrap: "wrap", gap: "0.75rem", fontSize: "0.875rem" }}
              >
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
                      padding: "0.125rem 0.5rem",
                      borderRadius: "4px",
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
        <section className="output-section" style={{ marginBottom: "1.5rem" }}>
          <h2 style={{ fontSize: "1.125rem" }}>{t("output.follow_up_draft")}</h2>
          <div className="follow-up-preview" style={{ marginTop: "0.5rem" }}>
            {followUp.subject && (
              <p style={{ marginTop: 0 }}>
                <strong>{t("output.subject")}:</strong> {followUp.subject}
              </p>
            )}
            <pre
              className="email-body"
              style={{
                whiteSpace: "pre-wrap",
                padding: "0.75rem",
                background: "#fafafa",
                borderRadius: "6px",
                border: "1px solid #e5e5e5",
                fontFamily: "inherit",
                fontSize: "0.875rem",
              }}
            >
              {followUpText}
            </pre>
          </div>
          <button
            type="button"
            style={{ marginTop: "0.75rem" }}
            onClick={() => {
              void navigator.clipboard.writeText(followUpText);
            }}
          >
            {t("output.copy_email")}
          </button>
        </section>
      )}

      <section className="output-section" style={{ marginBottom: "1.5rem" }}>
        <details>
          <summary style={{ cursor: "pointer" }}>{t("output.transcript")}</summary>
          <pre
            className="transcript"
            style={{
              whiteSpace: "pre-wrap",
              marginTop: "0.5rem",
              padding: "0.75rem",
              background: "#fafafa",
              borderRadius: "6px",
              maxHeight: "24rem",
              overflow: "auto",
            }}
          >
            {meeting.transcript}
          </pre>
        </details>
      </section>

      {output.participants_mentioned.length > 0 && (
        <section className="output-section">
          <p style={{ margin: 0 }}>
            {t("output.participants")}: {output.participants_mentioned.join(", ")}
          </p>
        </section>
      )}
    </div>
  );
}
