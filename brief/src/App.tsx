import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HistoryView } from "./views/HistoryView";
import { OutputView } from "./views/OutputView";
import { RecordingView } from "./views/RecordingView";
import { SettingsView } from "./views/SettingsView";

type AppView = "recording" | "output" | "history" | "settings";

export default function App() {
  const [view, setView] = useState<AppView>("recording");
  const invokeSmokeRan = useRef(false);

  useEffect(() => {
    if (invokeSmokeRan.current) {
      return;
    }
    invokeSmokeRan.current = true;
    void (async () => {
      await invoke("start_recording", { meeting_type: "internal" });
      await invoke("stop_recording", { session_id: "test" });
      await invoke("process_meeting", {
        session_id: "test",
        audio_path: "/tmp/x.wav",
      });
      await invoke("get_meeting", { id: "test" });
    })();
  }, []);

  return (
    <div style={{ padding: "1rem", fontFamily: "system-ui, sans-serif" }}>
      <h1>Brief</h1>
      <nav style={{ display: "flex", gap: "0.5rem", marginBottom: "1rem" }}>
        {(
          [
            ["recording", "Aufnahme"],
            ["output", "Ausgabe"],
            ["history", "Verlauf"],
            ["settings", "Einstellungen"],
          ] as const
        ).map(([key, label]) => (
          <button
            key={key}
            type="button"
            onClick={() => setView(key)}
            aria-current={view === key ? "page" : undefined}
          >
            {label}
          </button>
        ))}
      </nav>
      {view === "recording" && <RecordingView />}
      {view === "output" && <OutputView />}
      {view === "history" && <HistoryView />}
      {view === "settings" && <SettingsView />}
    </div>
  );
}
