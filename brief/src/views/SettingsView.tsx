import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import i18n from "../i18n";
import type { AppSettingsSnapshot, PersistedSettings } from "../types";

const DEFAULTS: PersistedSettings = {
  ollama_url: "http://localhost:11434",
  llm_model: "llama3.1:8b",
  default_meeting_type: "consulting",
  meeting_language: "de",
  retain_audio: "false",
  retention_days: "365",
  ui_language: "de",
};

function mergeSettings(raw: Record<string, string>): PersistedSettings {
  return {
    ollama_url: raw.ollama_url ?? DEFAULTS.ollama_url,
    llm_model: raw.llm_model ?? DEFAULTS.llm_model,
    default_meeting_type: raw.default_meeting_type ?? DEFAULTS.default_meeting_type,
    meeting_language: raw.meeting_language ?? DEFAULTS.meeting_language,
    retain_audio: raw.retain_audio ?? DEFAULTS.retain_audio,
    retention_days: raw.retention_days ?? DEFAULTS.retention_days,
    ui_language: raw.ui_language ?? DEFAULTS.ui_language,
  };
}

export function SettingsView() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<PersistedSettings | null>(null);
  const [snapshot, setSnapshot] = useState<AppSettingsSnapshot | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    void Promise.all([
      invoke<string>("get_all_settings").then((r) =>
        mergeSettings(JSON.parse(r) as Record<string, string>),
      ),
      invoke<AppSettingsSnapshot>("get_app_settings_snapshot"),
    ])
      .then(([s, snap]) => {
        setSettings(s);
        setSnapshot(snap);
      })
      .catch(() => {
        setSettings(null);
        setSnapshot(null);
      });
  }, []);

  const updateSetting = async (key: keyof PersistedSettings, value: string) => {
    await invoke("update_setting", { key, value });
    setSettings((prev) => (prev ? { ...prev, [key]: value } : prev));

    if (key === "llm_model") {
      void invoke<AppSettingsSnapshot>("get_app_settings_snapshot")
        .then(setSnapshot)
        .catch(() => {});
    }

    if (key === "ui_language") {
      void i18n.changeLanguage(value);
    }

    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  if (!settings) {
    return (
      <section aria-label={t("nav.settings")}>
        <p>{t("settings.loading")}</p>
      </section>
    );
  }

  return (
    <section className="settings-view" aria-label={t("nav.settings")}>
      <h2 style={{ marginTop: 0 }}>{t("settings.title")}</h2>
      {saved && (
        <p className="saved-indicator" role="status">
          {t("settings.saved")}
        </p>
      )}

      {snapshot && (
        <p>{t("settings.memory_detected", { gb: snapshot.memoryGb.toFixed(1) })}</p>
      )}

      <section style={{ marginBottom: "1.25rem" }}>
        <h3 style={{ marginBottom: "0.5rem" }}>{t("settings.ai_section")}</h3>

        <label style={{ display: "block", marginBottom: "0.75rem" }}>
          {t("settings.ollama_url")}
          <input
            value={settings.ollama_url}
            onChange={(e) => void updateSetting("ollama_url", e.target.value)}
            style={{ display: "block", width: "min(100%, 24rem)", marginTop: "0.25rem" }}
            autoComplete="off"
            spellCheck={false}
          />
        </label>

        <label style={{ display: "block", marginBottom: "0.5rem" }}>
          {t("settings.llm_model")}
          <input
            value={settings.llm_model}
            onChange={(e) => void updateSetting("llm_model", e.target.value)}
            style={{ display: "block", width: "min(100%, 24rem)", marginTop: "0.25rem" }}
            autoComplete="off"
            spellCheck={false}
          />
          <small style={{ display: "block", marginTop: "0.25rem" }}>
            {t("settings.llm_model_hint")}
          </small>
        </label>
        {snapshot?.llmModelUserOverride && (
          <p>{t("settings.llm_model_override_hint")}</p>
        )}
      </section>

      <section style={{ marginBottom: "1.25rem" }}>
        <h3 style={{ marginBottom: "0.5rem" }}>{t("settings.recording_section")}</h3>

        <label style={{ display: "block", marginBottom: "0.75rem" }}>
          {t("settings.meeting_language")}
          <select
            value={settings.meeting_language}
            onChange={(e) => void updateSetting("meeting_language", e.target.value)}
            style={{ display: "block", marginTop: "0.25rem" }}
          >
            <option value="de">Deutsch</option>
            <option value="en">English</option>
          </select>
        </label>

        <label style={{ display: "block", marginBottom: "0.75rem" }}>
          {t("settings.default_meeting_type")}
          <select
            value={settings.default_meeting_type}
            onChange={(e) => void updateSetting("default_meeting_type", e.target.value)}
            style={{ display: "block", marginTop: "0.25rem" }}
          >
            <option value="consulting">{t("meeting_types.consulting")}</option>
            <option value="legal">{t("meeting_types.legal")}</option>
            <option value="internal">{t("meeting_types.internal")}</option>
          </select>
        </label>

        <label style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <input
            type="checkbox"
            checked={settings.retain_audio === "true"}
            onChange={(e) =>
              void updateSetting("retain_audio", e.target.checked ? "true" : "false")
            }
          />
          {t("settings.retain_audio_label")}
        </label>
        <small style={{ display: "block", marginTop: "0.25rem" }}>
          {t("settings.retain_audio_description")}
        </small>
      </section>

      <section>
        <h3 style={{ marginBottom: "0.5rem" }}>{t("settings.app_section")}</h3>

        <label style={{ display: "block", marginBottom: "0.75rem" }}>
          {t("settings.ui_language")}
          <select
            value={settings.ui_language || "de"}
            onChange={(e) => void updateSetting("ui_language", e.target.value)}
            style={{ display: "block", marginTop: "0.25rem" }}
          >
            <option value="de">Deutsch</option>
            <option value="en">English</option>
          </select>
        </label>

        <label style={{ display: "block" }}>
          {t("settings.retention_days")}
          <input
            type="number"
            value={settings.retention_days}
            min={0}
            onChange={(e) => void updateSetting("retention_days", e.target.value)}
            style={{ display: "block", width: "min(100%, 12rem)", marginTop: "0.25rem" }}
          />
          <small style={{ display: "block", marginTop: "0.25rem" }}>
            {t("settings.retention_days_hint")}
          </small>
        </label>
      </section>
    </section>
  );
}
