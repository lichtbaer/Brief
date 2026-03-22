import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { TRANSCRIPTION_TIMEOUT_ERROR, type Meeting, type MeetingType } from "../types";

type AppStatus = "idle" | "recording" | "processing" | "done" | "error";
type ProcessingStep = "transcribing" | "summarizing";

interface RecordingViewProps {
  /** Called after a successful `process_meeting` when the parent should take over navigation (e.g. show output). */
  onMeetingDone?: (meeting: Meeting) => void;
}

function formatTime(seconds: number): string {
  const m = String(Math.floor(seconds / 60)).padStart(2, "0");
  const s = String(seconds % 60).padStart(2, "0");
  return `${m}:${s}`;
}

/**
 * Main recording flow: idle → recording → WhisperX/Ollama processing → done or error.
 * Invokes Tauri `start_recording`, `stop_recording`, and `process_meeting`; shows timers and transcript preview when inline.
 *
 * @param props.onMeetingDone — optional callback when processing finishes (parent-owned navigation).
 */
export function RecordingView({ onMeetingDone }: RecordingViewProps) {
  const { t } = useTranslation();
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [status, setStatus] = useState<AppStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [meetingType, setMeetingType] = useState<MeetingType>("consulting");
  const [elapsed, setElapsed] = useState(0);
  const [processingElapsed, setProcessingElapsed] = useState(0);
  const [processingStep, setProcessingStep] = useState<ProcessingStep | null>(null);

  // Elapsed timer during recording
  useEffect(() => {
    if (status !== "recording") {
      setElapsed(0);
      return;
    }
    const id = setInterval(() => setElapsed((s) => s + 1), 1000);
    return () => clearInterval(id);
  }, [status]);

  // Elapsed timer during WhisperX + summarization
  useEffect(() => {
    if (status !== "processing") {
      setProcessingElapsed(0);
      return;
    }
    const id = setInterval(() => setProcessingElapsed((s) => s + 1), 1000);
    return () => clearInterval(id);
  }, [status]);

  // Processing step hints
  useEffect(() => {
    if (status !== "processing") {
      setProcessingStep(null);
      return;
    }
    setProcessingStep("transcribing");
    const timer = setTimeout(() => setProcessingStep("summarizing"), 8000);
    return () => clearTimeout(timer);
  }, [status]);

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
        meeting_type: meetingType,
      });
      const parsed = JSON.parse(result) as Meeting;
      if (onMeetingDone) {
        onMeetingDone(parsed);
        reset();
      } else {
        setMeeting(parsed);
        setStatus("done");
      }
    } catch (err) {
      const raw = String(err);
      setError(
        raw.includes(TRANSCRIPTION_TIMEOUT_ERROR)
          ? t("errors.transcription_timeout")
          : raw,
      );
      setStatus("error");
    }
  };

  const startRecording = async () => {
    setError(null);
    setMeeting(null);
    try {
      const id = await invoke<string>("start_recording", {
        meeting_type: meetingType,
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

  const transcriptPreview =
    meeting?.transcript && meeting.transcript.length > 200
      ? `${meeting.transcript.slice(0, 200)}…`
      : (meeting?.transcript ?? "");

  const buttonClass =
    status === "idle"
      ? "btn btn-primary"
      : status === "recording"
        ? "btn btn-danger"
        : "btn btn-ghost";

  const buttonLabel =
    status === "idle"
      ? t("recording.btn_start")
      : status === "recording"
        ? t("recording.btn_stop")
        : status === "processing"
          ? t("recording.status_processing")
          : t("recording.btn_restart");

  return (
    <div style={{ padding: "2rem", maxWidth: "36rem" }}>
      <h1 style={{ marginBottom: "1.5rem", fontSize: "1.4rem", fontWeight: 700 }}>
        {t("recording.title")}
      </h1>

      {/* Meeting type selector — only visible when idle */}
      {status === "idle" && (
        <div className="form-group">
          <label className="form-label" htmlFor="meeting-type-select">
            {t("recording.meeting_type_label")}
          </label>
          <select
            id="meeting-type-select"
            className="form-select"
            value={meetingType}
            onChange={(e) => setMeetingType(e.target.value as MeetingType)}
          >
            <option value="consulting">{t("meeting_types.consulting")}</option>
            <option value="legal">{t("meeting_types.legal")}</option>
            <option value="internal">{t("meeting_types.internal")}</option>
          </select>
        </div>
      )}

      {/* Recording indicator with elapsed timer */}
      {status === "recording" && (
        <div className="record-status-bar">
          <span className="record-dot" />
          <span>{t("recording.status_recording")}</span>
          <span className="record-timer">{t("recording.elapsed", { time: formatTime(elapsed) })}</span>
        </div>
      )}

      {/* Processing indicator with step labels */}
      {status === "processing" && (
        <div className="processing-status">
          <span className="spinner spinner-dark" />
          <span>
            {processingStep === "transcribing"
              ? t("recording.step_transcribing")
              : t("recording.step_summarizing")}
          </span>
          <span className="record-timer">
            {t("recording.processing_elapsed", { time: formatTime(processingElapsed) })}
          </span>
        </div>
      )}

      {/* Error alert */}
      {error && (
        <div className="alert alert-error" role="alert">
          <span>⚠</span>
          <span>{t("errors.alert", { message: error })}</span>
        </div>
      )}

      {/* Transcript preview after done */}
      {meeting && status === "done" && (
        <div
          aria-label={t("recording.aria_transcript")}
          style={{
            marginBottom: "1rem",
            padding: "0.75rem 1rem",
            background: "#f7fafc",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--color-border)",
            fontSize: "0.9rem",
          }}
        >
          <p style={{ marginBottom: "0.25rem" }}>
            <strong>{t("recording.meeting_title_label")}</strong> {meeting.title}
          </p>
          <p style={{ color: "var(--color-text-muted)", whiteSpace: "pre-wrap" }}>
            {transcriptPreview}
          </p>
        </div>
      )}

      {/* Primary action button */}
      <button
        type="button"
        className={buttonClass}
        onClick={onPrimaryClick}
        disabled={status === "processing"}
        style={{ marginTop: "0.5rem" }}
      >
        {status === "processing" ? null : buttonLabel}
      </button>
    </div>
  );
}
