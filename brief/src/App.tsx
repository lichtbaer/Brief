import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { LowRamOnboardingBanner } from "./components/LowRamOnboardingBanner";
import { OnboardingWizard } from "./components/OnboardingWizard";
import { RecoveryBanner } from "./components/RecoveryBanner";
import i18n from "./i18n";
import type { AppSettingsSnapshot, Meeting, OrphanedRecording } from "./types";
import { HistoryView } from "./views/HistoryView";
import { OutputView } from "./views/OutputView";
import { RecordingView } from "./views/RecordingView";
import { SettingsView } from "./views/SettingsView";

type AppView = "recording" | "output" | "history" | "settings";

export default function App() {
  const { t } = useTranslation();
  const [onboardingComplete, setOnboardingComplete] = useState<boolean | null>(
    null,
  );
  const [currentView, setCurrentView] = useState<AppView>("recording");
  const [currentMeeting, setCurrentMeeting] = useState<Meeting | null>(null);
  const [settingsSnapshot, setSettingsSnapshot] =
    useState<AppSettingsSnapshot | null>(null);
  const [orphanRecording, setOrphanRecording] =
    useState<OrphanedRecording | null>(null);

  // Search query that caused the current meeting to be opened — passed to OutputView
  // so the transcript highlights the matched terms (Feature 6).
  const [meetingSearchQuery, setMeetingSearchQuery] = useState<string | undefined>(undefined);

  // Participant filter for the history view — set when the user clicks a participant name
  // in OutputView and navigates back to history (Feature 7).
  const [participantFilter, setParticipantFilter] = useState<string | undefined>(undefined);

  const handleMeetingDone = (meeting: Meeting) => {
    setMeetingSearchQuery(undefined);
    setCurrentMeeting(meeting);
    setCurrentView("output");
  };

  const handleOpenMeeting = (meeting: Meeting, fromSearchQuery?: string) => {
    setMeetingSearchQuery(fromSearchQuery);
    setCurrentMeeting(meeting);
    setCurrentView("output");
  };

  const handleOutputBack = () => {
    // Return to history when coming from a search or participant filter so context is preserved.
    setCurrentView(meetingSearchQuery || participantFilter ? "history" : "recording");
  };

  /** Called when the user clicks a participant name in OutputView — navigates to history filtered by that person. */
  const handleFilterByParticipant = (name: string) => {
    setParticipantFilter(name);
    setCurrentMeeting(null);
    setCurrentView("history");
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
        setOnboardingComplete(parsed.onboarding_complete === "true");
      })
      .catch(() => {
        setOnboardingComplete(true);
      });
  }, []);

  useEffect(() => {
    if (onboardingComplete !== true) {
      return;
    }
    void invoke<OrphanedRecording[]>("check_orphaned_recordings")
      .then((rows) => {
        if (rows.length > 0) {
          setOrphanRecording(rows[0]);
        }
      })
      .catch(() => {
        setOrphanRecording(null);
      });
  }, [onboardingComplete]);

  const handleLowRamDismissed = () => {
    setSettingsSnapshot((prev) =>
      prev ? { ...prev, showLowRamOnboarding: false } : null,
    );
  };

  if (onboardingComplete === null) {
    return (
      <div className="app-loading" role="status" aria-live="polite">
        <span className="spinner spinner-dark" />
        {t("onboarding.loading_app")}
      </div>
    );
  }

  if (!onboardingComplete) {
    return (
      <div className="onboarding-app-shell">
        <OnboardingWizard onComplete={() => setOnboardingComplete(true)} />
      </div>
    );
  }

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
        {orphanRecording && (
          <RecoveryBanner
            recording={orphanRecording}
            onRecovered={(meeting) => {
              setOrphanRecording(null);
              handleMeetingDone(meeting);
            }}
            onDismissBanner={() => setOrphanRecording(null)}
          />
        )}
        {settingsSnapshot?.showLowRamOnboarding === true && (
          <LowRamOnboardingBanner
            recommendedModel={settingsSnapshot.recommendedModel}
            onDismissed={handleLowRamDismissed}
          />
        )}
        <div key={currentView} className="view-enter">
          {currentView === "recording" && (
            <RecordingView onMeetingDone={handleMeetingDone} />
          )}
          {currentView === "output" && currentMeeting && (
            <OutputView
              meeting={currentMeeting}
              onBack={handleOutputBack}
              onMeetingUpdated={setCurrentMeeting}
              searchQuery={meetingSearchQuery}
              onFilterByParticipant={handleFilterByParticipant}
            />
          )}
          {currentView === "history" && (
            <HistoryView
              onOpenMeeting={handleOpenMeeting}
              initialParticipantFilter={participantFilter}
              onParticipantFilterConsumed={() => setParticipantFilter(undefined)}
            />
          )}
          {currentView === "settings" && <SettingsView />}
        </div>
      </main>
    </div>
  );
}
