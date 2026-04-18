import { useTranslation } from "react-i18next";
import type { FollowUpDraft } from "../types";

interface FollowUpSectionProps {
  /** Parsed follow-up draft from MeetingOutput (subject, body, full_text). */
  followUp: FollowUpDraft;
  /** The full_text value from the draft, pre-trimmed for empty checks. */
  followUpText: string;
  /** Non-null when the user is actively editing; null means read-only view. */
  followUpEditText: string | null;
  /** Whether the last save attempt failed. */
  followUpSaveError: boolean;
  /** Whether the clipboard copy just succeeded (drives the "Copied" flash). */
  copied: boolean;
  onChangeEditText: (text: string) => void;
  onSave: () => void;
  onCancel: () => void;
  onStartEdit: () => void;
  onCopy: () => void;
}

/**
 * FollowUpSection — renders the AI-generated follow-up email draft with
 * inline editing, clipboard copy, and backend persistence.
 * Shown only when the meeting output contains a non-empty full_text field.
 */
export function FollowUpSection({
  followUp,
  followUpText,
  followUpEditText,
  followUpSaveError,
  copied,
  onChangeEditText,
  onSave,
  onCancel,
  onStartEdit,
  onCopy,
}: FollowUpSectionProps) {
  const { t } = useTranslation();

  if (followUpText.length === 0) return null;

  return (
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
            onChange={(e) => onChangeEditText(e.target.value)}
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
            <button type="button" className="btn btn-ghost btn-icon" onClick={onStartEdit}>
              {t("output.follow_up_edit")}
            </button>
            <button type="button" className="btn btn-ghost btn-icon" onClick={onCopy}>
              {copied ? t("output.copied") : t("output.copy_email")}
            </button>
          </>
        ) : (
          <>
            <button type="button" className="btn btn-ghost btn-icon" onClick={onSave}>
              {t("output.follow_up_save")}
            </button>
            <button type="button" className="btn btn-ghost btn-icon" onClick={onCancel}>
              {t("output.follow_up_cancel")}
            </button>
            <button type="button" className="btn btn-ghost btn-icon" onClick={onCopy}>
              {copied ? t("output.copied") : t("output.copy_email")}
            </button>
          </>
        )}
      </div>
    </section>
  );
}
