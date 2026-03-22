import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import type { MeetingType } from "../types";

type RecordingStatus = "idle" | "recording" | "processing" | "done" | "error";

export function RecordingView() {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [status, setStatus] = useState<RecordingStatus>("idle");
  const [audioPath, setAudioPath] = useState<string | null>(null);
  const [transcript, setTranscript] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [meetingType] = useState<MeetingType>("consulting");

  const statusLabel: Record<RecordingStatus, string> = {
    idle: "Bereit",
    recording: "Aufnahme",
    processing: "Transkription läuft",
    done: "Fertig",
    error: "Fehler",
  };

  const startRecording = async () => {
    setError(null);
    setAudioPath(null);
    setTranscript(null);
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

  const processMeeting = async (sid: string, path: string) => {
    setStatus("processing");
    try {
      const result = await invoke<string>("process_meeting", {
        session_id: sid,
        audio_path: path,
      });
      const data = JSON.parse(result) as { transcript: string };
      setTranscript(data.transcript);
      setStatus("done");
    } catch (err) {
      setError(String(err));
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
      setSessionId(null);
      await processMeeting(sid, path);
    } catch (e) {
      setError(String(e));
      setStatus("error");
      setSessionId(null);
    }
  };

  return (
    <section aria-label="Aufnahme">
      <p>
        Status: {statusLabel[status]}
        {status === "recording" ? " 🔴" : ""}
      </p>
      {audioPath && <p>WAV gespeichert: {audioPath}</p>}
      {transcript && (
        <p>
          <strong>Transkript:</strong> {transcript}
        </p>
      )}
      {error && <p role="alert">Fehler: {error}</p>}
      <button
        type="button"
        onClick={status === "recording" ? stopRecording : startRecording}
        disabled={status === "processing"}
      >
        {status === "recording" ? "Stoppen" : "Aufnahme starten"}
      </button>
    </section>
  );
}
