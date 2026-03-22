import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { LowRamOnboardingBanner } from "./components/LowRamOnboardingBanner";
import i18n from "./i18n";
import type { AppSettingsSnapshot, Meeting } from "./types";
import { HistoryView } from "./views/HistoryView";
import { OutputView } from "./views/OutputView";
import { RecordingView } from "./views/RecordingView";
import { SettingsView } from "./views/SettingsView";

type AppView = "recording" | "output" | "history" | "settings";

export default function App() {
  const { t } = useTranslation();
  const [currentView, setCurrentView] = useState<AppView>("recording");
  const [currentMeeting, setCurrentMeeting] = useState<Meeting | null>(null);
  const [settingsSnapshot, setSettingsSnapshot] =
    useState<AppSettingsSnapshot | null>(null);

  const handleMeetingDone = (meeting: Meeting) => {
    setCurrentMeeting(meeting);
    setCurrentView("output");
  };

  const handleOpenMeeting = (meeting: Meeting) => {
    setCurrentMeeting(meeting);
    setCurrentView("output");
  };

  const handleOutputBack = () => {
    setCurrentView("recording");
  };

  useEffect(() => {
    void invoke<AppSettingsSnapshot>("get_app_settings_snapshot")
      .then(setSettingsSnapshot)
      .catch(() => setSettingsSnapshot(null));
  }, []);

  useEffect(() => {
    void invoke<string>("get_all_settings")
      .then((raw) => {
        const parsed = JSON.parse(raw) as Record<string, string>;
        const lang = parsed.ui_language;
        if (lang === "en" || lang === "de") {
          void i18n.changeLanguage(lang);
        }
      })
      .catch(() => {});
  }, []);

  const handleLowRamDismissed = () => {
    setSettingsSnapshot((prev) =>
      prev ? { ...prev, showLowRamOnboarding: false } : null,
    );
  };

  return (
    <div className="app-layout">
      <nav className="sidebar" aria-label={t("app.title")}>
        <div className="app-logo">
          <span>{t("app.title")}</span>
        </div>
        <button
          type="button"
          className={
            currentView === "recording" ? "nav-item active" : "nav-item"
          }
          onClick={() => setCurrentView("recording")}
          aria-current={currentView === "recording" ? "page" : undefined}
        >
          🎙 {t("nav.record")}
        </button>
        <button
          type="button"
          className={currentView === "history" ? "nav-item active" : "nav-item"}
          onClick={() => setCurrentView("history")}
          aria-current={currentView === "history" ? "page" : undefined}
        >
          📋 {t("nav.history")}
        </button>
        <button
          type="button"
          className={
            currentView === "settings" ? "nav-item active" : "nav-item"
          }
          onClick={() => setCurrentView("settings")}
          aria-current={currentView === "settings" ? "page" : undefined}
        >
          ⚙️ {t("nav.settings")}
        </button>
      </nav>

      <main className="main-content">
        {settingsSnapshot?.showLowRamOnboarding === true && (
          <LowRamOnboardingBanner
            recommendedModel={settingsSnapshot.recommendedModel}
            onDismissed={handleLowRamDismissed}
          />
        )}
        {currentView === "recording" && (
          <RecordingView onMeetingDone={handleMeetingDone} />
        )}
        {currentView === "output" && currentMeeting && (
          <OutputView
            meeting={currentMeeting}
            onBack={handleOutputBack}
          />
        )}
        {currentView === "history" && (
          <HistoryView onOpenMeeting={handleOpenMeeting} />
        )}
        {currentView === "settings" && <SettingsView />}
      </main>
    </div>
  );
}
