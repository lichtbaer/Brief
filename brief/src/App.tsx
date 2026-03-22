import { useState } from "react";
import { HistoryView } from "./views/HistoryView";
import { OutputView } from "./views/OutputView";
import { RecordingView } from "./views/RecordingView";
import { SettingsView } from "./views/SettingsView";

type AppView = "recording" | "output" | "history" | "settings";

export default function App() {
  const [view, setView] = useState<AppView>("recording");

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
