import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { MeetingType } from "../types";

type Segment = {
  speaker: string;
  start: number;
  end: number;
  text: string;
};

type RecordingStatus = "idle" | "recording" | "processing" | "done" | "error";

export function RecordingView() {
  const { t } = useTranslation();
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [status, setStatus] = useState<RecordingStatus>("idle");
  const [segments, setSegments] = useState<Segment[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [meetingType] = useState<MeetingType>("consulting");

  const processMeeting = async (sid: string, audioPath: string) => {
    setStatus("processing");
    try {
      const result = await invoke<string>("process_meeting", {
        session_id: sid,
        audio_path: audioPath,
        meeting_type: meetingType,
        title: null,
      });
      const data = JSON.parse(result) as { segments: Segment[] };
      setSegments(data.segments);
      setStatus("done");
    } catch (err) {
      setError(String(err));
      setStatus("error");
    }
  };

  const startRecording = async () => {
    setError(null);
    setSegments([]);
    setStatus("idle");
    try {
      const id = await invoke<string>("start_recording", {
        meeting_type: meetingType,
      });
      setSessionId(id);
      setIsRecording(true);
      setStatus("recording");
    } catch (e) {
      setError(String(e));
      setStatus("error");
    }
  };

  const stopRecording = async () => {
    if (!sessionId) return;
    setError(null);
    const currentSessionId = sessionId;
    try {
      const path = await invoke<string>("stop_recording", {
        session_id: currentSessionId,
      });
      setIsRecording(false);
      setSessionId(null);
      await processMeeting(currentSessionId, path);
    } catch (e) {
      setError(String(e));
      setStatus("error");
      setIsRecording(false);
      setSessionId(null);
    }
  };

  const statusLabel: Record<RecordingStatus, string> = {
    idle: t("recording.status_idle"),
    recording: t("recording.status_recording"),
    processing: t("recording.status_processing"),
    done: t("recording.status_done"),
    error: t("recording.status_error"),
  };

  return (
    <section aria-label={t("recording.aria_section")}>
      <p>
        {t("recording.status_label_prefix")}
        {statusLabel[status]}
      </p>
      {segments.length > 0 && status === "done" && (
        <div aria-label={t("recording.aria_transcript")}>
          {segments.map((seg, i) => (
            <p key={i}>
              <strong>{t("output.speaker_label", { speaker: seg.speaker })}</strong> {seg.text}
            </p>
          ))}
        </div>
      )}
      {error && (
        <p role="alert">{t("errors.alert", { message: error })}</p>
      )}
      <button type="button" onClick={isRecording ? stopRecording : startRecording}>
        {isRecording ? t("recording.btn_stop") : t("recording.btn_start")}
      </button>
    </section>
  );
}
