import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { TRANSCRIPTION_TIMEOUT_ERROR, type Meeting, type OrphanedRecording } from "../types";

type RecoveryBannerProps = {
  recording: OrphanedRecording;
  /** Called after successful recovery with the new meeting (parent navigates to output). */
  onRecovered: (meeting: Meeting) => void;
  /** Hide the banner (keep file, or after successful delete). */
  onDismissBanner: () => void;
};

/**
 * Full-width notice when a temp WAV from a crashed session is found.
 * Offers transcribe (default consulting title), keep, or explicit delete — never silent removal.
 */
export function RecoveryBanner({
  recording,
  onRecovered,
  onDismissBanner,
}: RecoveryBannerProps) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleTranscribe = async () => {
    setError(null);
    setBusy(true);
    try {
      const raw = await invoke<string>("recover_orphaned_recording", {
        audioPath: recording.path,
      });
      const meeting = JSON.parse(raw) as Meeting;
      onRecovered(meeting);
    } catch (err) {
      const raw = String(err);
      setError(
        raw.includes(TRANSCRIPTION_TIMEOUT_ERROR)
          ? t("errors.transcription_timeout")
          : raw,
      );
    } finally {
      setBusy(false);
    }
  };

  const handleDiscard = async () => {
    setError(null);
    setBusy(true);
    try {
      await invoke("discard_orphaned_recording", {
        audioPath: recording.path,
      });
      onDismissBanner();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside
      className="recovery-banner"
      role="alertdialog"
      aria-labelledby="recovery-banner-title"
      aria-describedby="recovery-banner-desc"
    >
      <h2 id="recovery-banner-title" className="recovery-banner__title">
        {t("recovery.title")}
      </h2>
      <p id="recovery-banner-desc" className="recovery-banner__meta">
        {recording.filename}{" "}
        <span className="recovery-banner__size">
          {t("recovery.size_inline", { mb: recording.size_mb })}
        </span>
      </p>
      <p className="recovery-banner__question">{t("recovery.question")}</p>
      {error && (
        <div className="alert alert-error recovery-banner__error" role="alert">
          {t("errors.alert", { message: error })}
        </div>
      )}
      <div className="recovery-banner__actions">
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void handleTranscribe()}
          disabled={busy}
        >
          {busy ? t("recovery.transcribing") : t("recovery.transcribe")}
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          onClick={onDismissBanner}
          disabled={busy}
        >
          {t("recovery.keep")}
        </button>
        <button
          type="button"
          className="btn btn-danger"
          onClick={() => void handleDiscard()}
          disabled={busy}
        >
          {t("recovery.discard")}
        </button>
      </div>
    </aside>
  );
}
