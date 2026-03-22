import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import type { MeetingType } from "../types";

type Segment = {
  speaker: string;
  start: number;
  end: number;
  text: string;
};

type ProcessStatus = "idle" | "recording" | "processing" | "done" | "error";

export function RecordingView() {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [audioPath, setAudioPath] = useState<string | null>(null);
  const [segments, setSegments] = useState<Segment[]>([]);
  const [status, setStatus] = useState<ProcessStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [meetingType] = useState<MeetingType>("consulting");

  const startRecording = async () => {
    setError(null);
    setAudioPath(null);
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
    const sid = sessionId;
    try {
      const path = await invoke<string>("stop_recording", {
        session_id: sid,
      });
      setAudioPath(path);
      setIsRecording(false);
      setSessionId(null);

      setStatus("processing");
      const result = await invoke<string>("process_meeting", {
        session_id: sid,
        audio_path: path,
      });
      const data = JSON.parse(result) as { segments: Segment[] };
      setSegments(data.segments);
      setStatus("done");
    } catch (e) {
      setError(String(e));
      setStatus("error");
    }
  };

  return (
    <section aria-label="Aufnahme">
      <p>Status: {isRecording ? "🔴 Aufnahme läuft" : "⚫ Bereit"}</p>
      {audioPath && <p>WAV gespeichert: {audioPath}</p>}
      {status === "processing" && <p>Transkription läuft …</p>}
      {error && <p role="alert">Fehler: {error}</p>}
      <button type="button" onClick={isRecording ? stopRecording : startRecording}>
        {isRecording ? "Stoppen" : "Aufnahme starten"}
      </button>
      {status === "done" &&
        segments.map((seg, i) => (
          <p key={i}>
            <strong>{seg.speaker}:</strong> {seg.text}
          </p>
        ))}
    </section>
  );
}
