import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { Meeting } from "../types";

type AppStatus = "idle" | "recording" | "processing" | "done" | "error";

export function RecordingView() {
  const { t } = useTranslation();
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [status, setStatus] = useState<AppStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [meeting, setMeeting] = useState<Meeting | null>(null);

  const reset = () => {
    setError(null);
    setMeeting(null);
    setSessionId(null);
    setStatus("idle");
  };

  const processMeeting = async (sid: string, audioPath: string) => {
    setStatus("processing");
    try {
      const result = await invoke<string>("process_meeting", {
        session_id: sid,
        audio_path: audioPath,
      });
      const parsed = JSON.parse(result) as Meeting;
      setMeeting(parsed);
      setStatus("done");
    } catch (err) {
      setError(String(err));
      setStatus("error");
    }
  };

  const startRecording = async () => {
    setError(null);
    setMeeting(null);
    try {
      const id = await invoke<string>("start_recording", {
        meeting_type: "consulting",
      });
      setSessionId(id);
      setStatus("recording");
    } catch (e) {
      setError(String(e));
      setStatus("error");
    }
  };

  const stopAndProcess = async () => {
    if (!sessionId) return;
    setError(null);
    const currentSessionId = sessionId;
    try {
      const path = await invoke<string>("stop_recording", {
        session_id: currentSessionId,
      });
      setSessionId(null);
      await processMeeting(currentSessionId, path);
    } catch (e) {
      setError(String(e));
      setStatus("error");
      setSessionId(null);
    }
  };

  const onPrimaryClick = () => {
    if (status === "idle") {
      void startRecording();
      return;
    }
    if (status === "recording") {
      void stopAndProcess();
      return;
    }
    if (status === "done" || status === "error") {
      reset();
    }
  };

  const statusLabel: Record<AppStatus, string> = {
    idle: t("recording.status_idle"),
    recording: t("recording.status_recording"),
    processing: t("recording.status_processing"),
    done: t("recording.status_done"),
    error: t("recording.status_error"),
  };

  const buttonLabel =
    status === "idle"
      ? t("recording.btn_start")
      : status === "recording"
        ? t("recording.btn_stop")
        : status === "processing"
          ? t("recording.status_processing")
          : t("recording.btn_restart");

  const transcriptPreview =
    meeting?.transcript && meeting.transcript.length > 200
      ? `${meeting.transcript.slice(0, 200)}…`
      : (meeting?.transcript ?? "");

  return (
    <div
      style={{
        padding: "2rem",
        fontFamily: "monospace",
      }}
    >
      <h1>{t("recording.smoke_test_heading")}</h1>
      <p>
        {t("recording.status_label_prefix")}
        <strong>{statusLabel[status]}</strong>
      </p>
      {error && (
        <p style={{ color: "red" }} role="alert">
          {t("errors.alert", { message: error })}
        </p>
      )}
      {meeting && status === "done" && (
        <div aria-label={t("recording.aria_transcript")}>
          <p>
            {t("recording.meeting_id_label")} {meeting.id}
          </p>
          <p>
            {t("recording.transcript_preview_label")} {transcriptPreview}
          </p>
          <p>
            {t("recording.meeting_title_label")} {meeting.title}
          </p>
        </div>
      )}
      <button
        type="button"
        onClick={onPrimaryClick}
        disabled={status === "processing"}
      >
        {buttonLabel}
      </button>
    </div>
  );
}
