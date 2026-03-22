import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { LowRamOnboardingBanner } from "./components/LowRamOnboardingBanner";
import type { AppSettingsSnapshot, Meeting } from "./types";
import { HistoryView } from "./views/HistoryView";
import { OutputView } from "./views/OutputView";
import { RecordingView } from "./views/RecordingView";
import { SettingsView } from "./views/SettingsView";

type AppView = "recording" | "output" | "history" | "settings";

export default function App() {
  const { t } = useTranslation();
  const [view, setView] = useState<AppView>("recording");
  const [currentMeeting, setCurrentMeeting] = useState<Meeting | null>(null);
  const [settingsSnapshot, setSettingsSnapshot] =
    useState<AppSettingsSnapshot | null>(null);

  const handleMeetingDone = (meeting: Meeting) => {
    setCurrentMeeting(meeting);
    setView("output");
  };

  const handleOutputBack = () => {
    setView("recording");
  };

  useEffect(() => {
    void invoke<AppSettingsSnapshot>("get_app_settings_snapshot")
      .then(setSettingsSnapshot)
      .catch(() => setSettingsSnapshot(null));
  }, []);

  const handleLowRamDismissed = () => {
    setSettingsSnapshot((prev) =>
      prev ? { ...prev, showLowRamOnboarding: false } : null,
    );
  };

  return (
    <div style={{ padding: "1rem", fontFamily: "system-ui, sans-serif" }}>
      <h1>{t("app.title")}</h1>
      {settingsSnapshot?.showLowRamOnboarding === true && (
        <LowRamOnboardingBanner
          recommendedModel={settingsSnapshot.recommendedModel}
          onDismissed={handleLowRamDismissed}
        />
      )}
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
      {view === "recording" && (
        <RecordingView onMeetingDone={handleMeetingDone} />
      )}
      {view === "output" &&
        (currentMeeting ? (
          <OutputView meeting={currentMeeting} onBack={handleOutputBack} />
        ) : (
          <p role="status">{t("output.empty")}</p>
        ))}
      {view === "history" && <HistoryView />}
      {view === "settings" && <SettingsView />}
    </div>
  );
}
