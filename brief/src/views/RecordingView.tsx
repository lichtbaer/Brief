import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import type { MeetingType } from "../types";

export function RecordingView() {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [audioPath, setAudioPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [meetingType] = useState<MeetingType>("consulting");

  const startRecording = async () => {
    setError(null);
    setAudioPath(null);
    try {
      const id = await invoke<string>("start_recording", {
        meeting_type: meetingType,
      });
      setSessionId(id);
      setIsRecording(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const stopRecording = async () => {
    if (!sessionId) return;
    setError(null);
    try {
      const path = await invoke<string>("stop_recording", {
        session_id: sessionId,
      });
      setAudioPath(path);
      setIsRecording(false);
      setSessionId(null);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <section aria-label="Aufnahme">
      <p>Status: {isRecording ? "🔴 Aufnahme läuft" : "⚫ Bereit"}</p>
      {audioPath && <p>WAV gespeichert: {audioPath}</p>}
      {error && <p role="alert">Fehler: {error}</p>}
      <button type="button" onClick={isRecording ? stopRecording : startRecording}>
        {isRecording ? "Stoppen" : "Aufnahme starten"}
      </button>
    </section>
  );
}
