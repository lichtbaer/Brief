import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import type { MeetingType } from "../types";

type Segment = {
  speaker: string;
  start: number;
  end: number;
  text: string;
};

type RecordingStatus = "idle" | "recording" | "processing" | "done" | "error";

export function RecordingView() {
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
    idle: "⚫ Bereit",
    recording: "🔴 Aufnahme läuft",
    processing: "⏳ Transkription (WhisperX)…",
    done: "✅ Fertig",
    error: "❌ Fehler",
  };

  return (
    <section aria-label="Aufnahme">
      <p>Status: {statusLabel[status]}</p>
      {segments.length > 0 && status === "done" && (
        <div aria-label="Transkript">
          {segments.map((seg, i) => (
            <p key={i}>
              <strong>{seg.speaker}:</strong> {seg.text}
            </p>
          ))}
        </div>
      )}
      {error && <p role="alert">Fehler: {error}</p>}
      <button type="button" onClick={isRecording ? stopRecording : startRecording}>
        {isRecording ? "Stoppen" : "Aufnahme starten"}
      </button>
    </section>
  );
}
