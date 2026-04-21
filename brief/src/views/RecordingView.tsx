import { invoke } from "@tauri-apps/api/core";
import { updateSetting } from "../services/settingsService";
import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { TRANSCRIPTION_TIMEOUT_ERROR, isMeeting, type Meeting, type MeetingType } from "../types";

/**
 * Maps raw backend error strings to localized i18n keys.
 * Stable token constants are preferred over substring matches to survive refactoring.
 * Falls back to the raw message string so technical errors remain visible.
 */
function classifyError(raw: string, t: (key: string) => string): string {
  if (raw.includes(TRANSCRIPTION_TIMEOUT_ERROR)) return t("errors.transcription_timeout");
  // NoMicrophone error from audio.rs — stable variant name used as token.
  if (raw.toLowerCase().includes("no microphone") || raw.includes("NoMicrophone")) return t("errors.no_microphone");
  // Ollama not reachable — stable message from summarize.rs.
  if (raw.includes("Ollama not reachable")) return t("errors.ollama_not_reachable");
  // WhisperX subprocess failed to start.
  if (raw.includes("WhisperX") || raw.includes("whisperx")) return t("errors.whisperx_unavailable");
  return raw;
}
type AppStatus = "idle" | "recording" | "processing" | "done" | "error";
type ProcessingStep = "transcribing" | "summarizing";

interface RecordingViewProps {
  /** Called after a successful `process_meeting` when the parent should take over navigation (e.g. show output). */
  onMeetingDone?: (meeting: Meeting) => void;
}

export function formatTime(seconds: number): string {
  const m = String(Math.floor(seconds / 60)).padStart(2, "0");
  const s = String(seconds % 60).padStart(2, "0");
  return `${m}:${s}`;
}

// -- Consolidated recording state via useReducer --

interface RecordingState {
  status: AppStatus;
  sessionId: string | null;
  error: string | null;
  meeting: Meeting | null;
  processingStep: ProcessingStep | null;
}

type RecordingAction =
  | { type: "START_RECORDING"; sessionId: string }
  | { type: "START_PROCESSING" }
  | { type: "PROCESSING_DONE"; meeting: Meeting }
  | { type: "ERROR"; error: string }
  | { type: "SET_PROCESSING_STEP"; step: ProcessingStep }
  | { type: "CLEAR_SESSION" }
  | { type: "RESET" };

export const initialState: RecordingState = {
  status: "idle",
  sessionId: null,
  error: null,
  meeting: null,
  processingStep: null,
};

export function recordingReducer(state: RecordingState, action: RecordingAction): RecordingState {
  switch (action.type) {
    case "START_RECORDING":
      return { ...initialState, status: "recording", sessionId: action.sessionId };
    case "START_PROCESSING":
      return { ...state, status: "processing", sessionId: null, error: null, processingStep: "transcribing" };
    case "PROCESSING_DONE":
      return { ...state, status: "done", meeting: action.meeting, processingStep: null };
    case "ERROR":
      return { ...state, status: "error", error: action.error, sessionId: null, processingStep: null };
    case "SET_PROCESSING_STEP":
      return { ...state, processingStep: action.step };
    case "CLEAR_SESSION":
      return { ...state, sessionId: null };
    case "RESET":
      return initialState;
  }
}

/**
 * Main recording flow: idle → recording → WhisperX/Ollama processing → done or error.
 * Invokes Tauri `start_recording`, `stop_recording`, and `process_meeting`; shows timers and transcript preview when inline.
 * Meeting language (WhisperX) is chosen before recording and persisted as `meeting_language` in settings, separate from UI language.
 *
 * @param props.onMeetingDone — optional callback when processing finishes (parent-owned navigation).
 */
export function RecordingView({ onMeetingDone }: RecordingViewProps) {
  const { t } = useTranslation();
  const [state, dispatch] = useReducer(recordingReducer, initialState);
  const [meetingType, setMeetingType] = useState<MeetingType>("consulting");
  const [meetingLanguage, setMeetingLanguage] = useState<string>("de");
  const [elapsed, setElapsed] = useState(0);
  const [processingElapsed, setProcessingElapsed] = useState(0);
  // Audio level [0, 1] polled from backend while recording to drive the level meter.
  const [audioLevel, setAudioLevel] = useState(0);
  // True when the recording buffer hit the ~4 hour cap and frames are being dropped.
  const [bufferOverflow, setBufferOverflow] = useState(false);
  // Threshold (seconds) after which the processing step label switches from "transcribing" to
  // "summarizing". useRef avoids re-renders; the value is only read inside a setInterval callback.
  const processingStepHintSecsRef = useRef(8);
  useEffect(() => {
    void invoke<{ processing_step_hint_secs: number }>("get_setting_defaults")
      .then((d) => { processingStepHintSecsRef.current = d.processing_step_hint_secs; })
      .catch(() => {});
  }, []);

  const { status, sessionId, error, meeting, processingStep } = state;

  // Load persisted meeting language (WhisperX) — independent of UI locale.
  useEffect(() => {
    void invoke<string>("get_all_settings")
      .then((r) => {
        const raw = JSON.parse(r) as Record<string, string>;
        setMeetingLanguage(raw.meeting_language ?? "de");
      })
      .catch(() => {});
  }, []);

  // Unified timer effect: manages recording elapsed, processing elapsed, and
  // processing step hint in a single interval.  Consolidating avoids multiple
  // independent intervals that could leak if one cleanup fails.
  useEffect(() => {
    if (status === "recording") {
      setElapsed(0);
      const id = setInterval(() => setElapsed((s) => s + 1), 1000);
      return () => clearInterval(id);
    }

    // Reset recording timer when not recording.
    setElapsed(0);

    if (status === "processing") {
      setProcessingElapsed(0);

      const id = setInterval(() => {
        setProcessingElapsed((s) => {
          // Switch the step hint once processing exceeds the configured threshold (heuristic:
          // WhisperX typically finishes within this window on modern hardware).
          if (s + 1 >= processingStepHintSecsRef.current) dispatch({ type: "SET_PROCESSING_STEP", step: "summarizing" });
          return s + 1;
        });
      }, 1000);

      return () => clearInterval(id);
    }

    // Reset processing state when neither recording nor processing.
    setProcessingElapsed(0);
  }, [status]);

  // Poll audio level at ~5 Hz while recording for the level meter visualisation.
  // A separate effect keeps this concern isolated from the timer logic above.
  useEffect(() => {
    if (status !== "recording" || !sessionId) {
      setAudioLevel(0);
      return;
    }
    const id = setInterval(() => {
      void invoke<{ level: number; buffer_overflow: boolean }>("get_audio_level", { sessionId })
        .then((status) => {
          setAudioLevel(status.level);
          if (status.buffer_overflow) setBufferOverflow(true);
        })
        .catch(() => {});
    }, 200);
    return () => clearInterval(id);
  }, [status, sessionId]);

  const processMeeting = useCallback(async (sid: string, audioPath: string) => {
    dispatch({ type: "START_PROCESSING" });
    try {
      const result = await invoke<string>("process_meeting", {
        sessionId: sid,
        audioPath: audioPath,
        meetingType: meetingType,
      });
      const parsed = JSON.parse(result) as unknown;
      if (!isMeeting(parsed)) throw new Error("Invalid meeting data from backend");
      if (onMeetingDone) {
        onMeetingDone(parsed);
        dispatch({ type: "RESET" });
      } else {
        dispatch({ type: "PROCESSING_DONE", meeting: parsed });
      }
    } catch (err) {
      dispatch({ type: "ERROR", error: classifyError(String(err), t) });
    }
  }, [meetingType, onMeetingDone, dispatch, t]);

  const startRecording = useCallback(async () => {
    try {
      setBufferOverflow(false);
      const id = await invoke<string>("start_recording", {
        meetingType: meetingType,
      });
      dispatch({ type: "START_RECORDING", sessionId: id });
    } catch (e) {
      dispatch({ type: "ERROR", error: classifyError(String(e), t) });
    }
  }, [meetingType, setBufferOverflow, dispatch, t]);

  const stopAndProcess = useCallback(async () => {
    if (!sessionId) return;
    const currentSessionId = sessionId;
    try {
      const path = await invoke<string>("stop_recording", {
        sessionId: currentSessionId,
      });
      await processMeeting(currentSessionId, path);
    } catch (e) {
      dispatch({ type: "ERROR", error: classifyError(String(e), t) });
    }
  }, [sessionId, processMeeting, dispatch, t]);

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
      dispatch({ type: "RESET" });
    }
  };

  // Keyboard shortcut: Space bar toggles recording start/stop.
  // Ignored when the focus is inside an interactive element (input, textarea, select, button)
  // to avoid conflicts with form interactions.
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code !== "Space") return;
      const tag = (e.target as HTMLElement).tagName.toLowerCase();
      if (["input", "textarea", "select", "button"].includes(tag)) return;
      e.preventDefault();
      if (status === "idle") void startRecording();
      else if (status === "recording") void stopAndProcess();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [status, startRecording, stopAndProcess]);

  // Characters shown in the post-recording transcript preview — kept as a named constant.
  const TRANSCRIPT_PREVIEW_CHARS = 200;
  const transcriptPreview =
    meeting?.transcript && meeting.transcript.length > TRANSCRIPT_PREVIEW_CHARS
      ? `${meeting.transcript.slice(0, TRANSCRIPT_PREVIEW_CHARS)}…`
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
        <>
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
              <option value="custom">{t("meeting_types.custom")}</option>
            </select>
          </div>
          <div className="form-group">
            <label className="form-label" htmlFor="recording-meeting-language">
              {t("settings.meeting_language")}
            </label>
            <select
              id="recording-meeting-language"
              className="form-select"
              value={meetingLanguage}
              onChange={(e) => {
                const v = e.target.value;
                setMeetingLanguage(v);
                void updateSetting("meeting_language", v).catch(() => {});
              }}
              style={{ maxWidth: "14rem" }}
            >
              <option value="de">{t("languages.de")}</option>
              <option value="en">{t("languages.en")}</option>
            </select>
          </div>
        </>
      )}

      {/* Recording indicator with elapsed timer and audio level meter */}
      {status === "recording" && (
        <>
          <div className="record-status-bar">
            <span className="record-dot" />
            <span>{t("recording.status_recording")}</span>
            <span className="record-timer">{t("recording.elapsed", { time: formatTime(elapsed) })}</span>
          </div>
          {/* Level meter: green bar showing current microphone RMS. Width scales with audioLevel. */}
          <div
            aria-label={t("recording.level_meter_aria")}
            style={{
              marginBottom: "0.75rem",
              height: "6px",
              borderRadius: "3px",
              background: "var(--color-border, #e5e7eb)",
              overflow: "hidden",
              maxWidth: "20rem",
            }}
          >
            <div
              style={{
                height: "100%",
                // Apply a sqrt curve so the meter is more visually responsive at low levels.
                width: `${Math.round(Math.sqrt(audioLevel) * 100)}%`,
                background: audioLevel > 0.8 ? "#ef4444" : audioLevel > 0.5 ? "#f59e0b" : "#22c55e",
                transition: "width 0.1s ease-out, background 0.1s ease-out",
                borderRadius: "3px",
              }}
            />
          </div>
          {/* Warning when recording buffer has hit the ~4 hour cap */}
          {bufferOverflow && (
            <div className="alert alert-error" role="alert" style={{ marginBottom: "0.75rem", fontSize: "0.85rem" }}>
              <span>⚠</span>
              <span>{t("recording.buffer_overflow_warning")}</span>
            </div>
          )}
        </>
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

      {/* Primary action button — shows spinner + label during processing to avoid empty button state */}
      <button
        type="button"
        className={buttonClass}
        onClick={onPrimaryClick}
        disabled={status === "processing"}
        style={{ marginTop: "0.5rem" }}
      >
        {status === "processing" ? (
          <><span className="spinner spinner-dark" />{buttonLabel}</>
        ) : (
          buttonLabel
        )}
      </button>
      {/* Space bar shortcut hint — shown when idle or recording */}
      {(status === "idle" || status === "recording") && (
        <p style={{ marginTop: "0.5rem", fontSize: "0.78rem", color: "var(--color-text-subtle)" }}>
          {t("recording.shortcut_hint")}
        </p>
      )}
    </div>
  );
}
