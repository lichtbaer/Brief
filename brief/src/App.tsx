import { useState } from "react";
import { useTranslation } from "react-i18next";
import { HistoryView } from "./views/HistoryView";
import { OutputView } from "./views/OutputView";
import { RecordingView } from "./views/RecordingView";
import { SettingsView } from "./views/SettingsView";

type AppView = "recording" | "output" | "history" | "settings";

export default function App() {
  const { t } = useTranslation();
  const [view, setView] = useState<AppView>("recording");

  return (
    <div style={{ padding: "1rem", fontFamily: "system-ui, sans-serif" }}>
      <h1>{t("app.title")}</h1>
      <nav style={{ display: "flex", gap: "0.5rem", marginBottom: "1rem" }}>
        {(
          [
            ["recording", "nav.recording"],
            ["output", "nav.output"],
            ["history", "nav.history"],
            ["settings", "nav.settings"],
          ] as const
        ).map(([key, labelKey]) => (
          <button
            key={key}
            type="button"
            onClick={() => setView(key)}
            aria-current={view === key ? "page" : undefined}
          >
            {t(labelKey)}
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
