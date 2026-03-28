import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import i18n from "../i18n";
import type { AppSettingsSnapshot, PersistedSettings, SettingDefaults } from "../types";

// Hardcoded fallback only used until the backend command resolves (first render).
const FALLBACK_DEFAULTS: PersistedSettings = {
  ollama_url: "http://localhost:11434",
  llm_model: "llama3.1:8b",
  default_meeting_type: "consulting",
  meeting_language: "de",
  retain_audio: "false",
  retention_days: "365",
  ui_language: "de",
  whisperx_timeout_secs: "900",
  ollama_timeout_secs: "300",
};

/** Merges raw DB settings with defaults from the Rust backend (single source of truth). */
export function mergeSettings(raw: Record<string, string>, defaults?: SettingDefaults): PersistedSettings {
  const d = defaults ?? FALLBACK_DEFAULTS;
  return {
    ollama_url: raw.ollama_url ?? d.ollama_url,
    llm_model: raw.llm_model ?? d.llm_model,
    default_meeting_type: raw.default_meeting_type ?? d.default_meeting_type,
    meeting_language: raw.meeting_language ?? d.meeting_language,
    retain_audio: raw.retain_audio ?? d.retain_audio,
    retention_days: raw.retention_days ?? d.retention_days,
    ui_language: raw.ui_language ?? d.ui_language,
    whisperx_timeout_secs: raw.whisperx_timeout_secs ?? d.whisperx_timeout_secs,
    ollama_timeout_secs: raw.ollama_timeout_secs ?? d.ollama_timeout_secs,
  };
}

/** @deprecated Use `mergeSettings` with backend defaults instead. Re-exported for test compatibility. */
export const DEFAULTS = FALLBACK_DEFAULTS;

/**
 * Persists app and AI settings (Ollama URL, models, WhisperX timeout, retain audio, UI language) via `get_all_settings` / `update_setting`.
 * Shows RAM-based model recommendation from `get_app_settings_snapshot`.
 */
export function SettingsView() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<PersistedSettings | null>(null);
  const [snapshot, setSnapshot] = useState<AppSettingsSnapshot | null>(null);
  const [saved, setSaved] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);

  useEffect(() => {
    void Promise.all([
      invoke<string>("get_all_settings"),
      invoke<AppSettingsSnapshot>("get_app_settings_snapshot"),
      invoke<SettingDefaults>("get_setting_defaults"),
    ])
      .then(([raw, snap, defaults]) => {
        setSettings(mergeSettings(JSON.parse(raw) as Record<string, string>, defaults));
        setSnapshot(snap);
      })
      .catch(() => {
        setSettings(null);
        setSnapshot(null);
      });
  }, []);

  const updateSetting = async (key: keyof PersistedSettings, value: string) => {
    try {
      await invoke("update_setting", { key, value });
    } catch (e) {
      setSettingsError(String(e));
      setTimeout(() => setSettingsError(null), 5000);
      return;
    }
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
        <p style={{ color: "var(--color-text-muted)" }}>{t("settings.loading")}</p>
      </section>
    );
  }

  return (
    <section style={{ maxWidth: "36rem" }} aria-label={t("nav.settings")}>
      <div style={{ display: "flex", alignItems: "center", gap: "1rem", marginBottom: "1.5rem" }}>
        <h1 style={{ fontSize: "1.4rem", fontWeight: 700 }}>{t("settings.title")}</h1>
        {saved && (
          <span
            className="alert alert-success"
            role="status"
            style={{ padding: "0.3rem 0.75rem", margin: 0, fontSize: "0.85rem" }}
          >
            {t("settings.saved")}
          </span>
        )}
      </div>

      {settingsError && (
        <div className="alert alert-error" role="alert" style={{ marginBottom: "1rem" }}>
          <span>{t("errors.alert", { message: settingsError })}</span>
        </div>
      )}

      {snapshot && (
        <p style={{ fontSize: "0.85rem", color: "var(--color-text-muted)", marginBottom: "1.5rem" }}>
          {t("settings.memory_detected", { gb: snapshot.memoryGb.toFixed(1) })}
        </p>
      )}

      {/* AI & Model section */}
      <section style={{ marginBottom: "2rem" }}>
        <h2 style={{ fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--color-text-subtle)", fontWeight: 600, marginBottom: "1rem" }}>
          {t("settings.ai_section")}
        </h2>

        <div className="form-group">
          <label className="form-label" htmlFor="ollama-url">
            {t("settings.ollama_url")}
          </label>
          <input
            id="ollama-url"
            className="form-input"
            value={settings.ollama_url}
            onChange={(e) => void updateSetting("ollama_url", e.target.value)}
            autoComplete="off"
            spellCheck={false}
            style={{ maxWidth: "24rem" }}
          />
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="llm-model">
            {t("settings.llm_model")}
          </label>
          <input
            id="llm-model"
            className="form-input"
            value={settings.llm_model}
            onChange={(e) => void updateSetting("llm_model", e.target.value)}
            autoComplete="off"
            spellCheck={false}
            style={{ maxWidth: "24rem" }}
          />
          <small style={{ display: "block", marginTop: "0.35rem", color: "var(--color-text-muted)", fontSize: "0.8rem" }}>
            {t("settings.llm_model_hint")}
          </small>
        </div>

        {snapshot?.llmModelUserOverride && (
          <p style={{ fontSize: "0.85rem", color: "var(--color-warning)", marginTop: "-0.5rem" }}>
            {t("settings.llm_model_override_hint")}
          </p>
        )}

        <div className="form-group">
          <label className="form-label" htmlFor="ollama-timeout-secs">
            {t("settings.ollama_timeout")}
          </label>
          <input
            id="ollama-timeout-secs"
            type="number"
            className="form-input"
            value={settings.ollama_timeout_secs ?? FALLBACK_DEFAULTS.ollama_timeout_secs}
            min={30}
            max={3600}
            onChange={(e) => void updateSetting("ollama_timeout_secs", e.target.value)}
            style={{ maxWidth: "10rem" }}
          />
          <small style={{ display: "block", marginTop: "0.35rem", color: "var(--color-text-muted)", fontSize: "0.8rem" }}>
            {t("settings.ollama_timeout_hint")}
          </small>
        </div>
      </section>

      {/* Recording section */}
      <section style={{ marginBottom: "2rem" }}>
        <h2 style={{ fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--color-text-subtle)", fontWeight: 600, marginBottom: "1rem" }}>
          {t("settings.recording_section")}
        </h2>

        <div className="form-group">
          <label className="form-label" htmlFor="meeting-language">
            {t("settings.meeting_language")}
          </label>
          <select
            id="meeting-language"
            className="form-select"
            value={settings.meeting_language}
            onChange={(e) => void updateSetting("meeting_language", e.target.value)}
            style={{ maxWidth: "14rem" }}
          >
            <option value="de">{t("languages.de")}</option>
            <option value="en">{t("languages.en")}</option>
          </select>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="whisperx-timeout-secs">
            {t("settings.whisperx_timeout")}
          </label>
          <input
            id="whisperx-timeout-secs"
            type="number"
            className="form-input"
            value={settings.whisperx_timeout_secs ?? FALLBACK_DEFAULTS.whisperx_timeout_secs}
            min={60}
            max={86400}
            onChange={(e) => void updateSetting("whisperx_timeout_secs", e.target.value)}
            style={{ maxWidth: "10rem" }}
          />
          <small style={{ display: "block", marginTop: "0.35rem", color: "var(--color-text-muted)", fontSize: "0.8rem" }}>
            {t("settings.whisperx_timeout_hint")}
          </small>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="default-meeting-type">
            {t("settings.default_meeting_type")}
          </label>
          <select
            id="default-meeting-type"
            className="form-select"
            value={settings.default_meeting_type}
            onChange={(e) => void updateSetting("default_meeting_type", e.target.value)}
            style={{ maxWidth: "20rem" }}
          >
            <option value="consulting">{t("meeting_types.consulting")}</option>
            <option value="legal">{t("meeting_types.legal")}</option>
            <option value="internal">{t("meeting_types.internal")}</option>
          </select>
        </div>

        <div className="form-group">
          <label style={{ display: "flex", alignItems: "center", gap: "0.5rem", cursor: "pointer" }}>
            <input
              type="checkbox"
              checked={settings.retain_audio === "true"}
              onChange={(e) =>
                void updateSetting("retain_audio", e.target.checked ? "true" : "false")
              }
            />
            <span style={{ fontSize: "0.9rem" }}>{t("settings.retain_audio_label")}</span>
          </label>
          <small style={{ display: "block", marginTop: "0.35rem", color: "var(--color-text-muted)", fontSize: "0.8rem", paddingLeft: "1.5rem" }}>
            {t("settings.retain_audio_description")}
          </small>
        </div>
      </section>

      {/* App section */}
      <section>
        <h2 style={{ fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--color-text-subtle)", fontWeight: 600, marginBottom: "1rem" }}>
          {t("settings.app_section")}
        </h2>

        <div className="form-group">
          <label className="form-label" htmlFor="ui-language">
            {t("settings.ui_language")}
          </label>
          <select
            id="ui-language"
            className="form-select"
            value={settings.ui_language ?? "de"}
            onChange={(e) => void updateSetting("ui_language", e.target.value)}
            style={{ maxWidth: "14rem" }}
          >
            <option value="de">{t("languages.de")}</option>
            <option value="en">{t("languages.en")}</option>
          </select>
        </div>

        <div className="form-group">
          <label className="form-label" htmlFor="retention-days">
            {t("settings.retention_days")}
          </label>
          <input
            id="retention-days"
            type="number"
            className="form-input"
            value={settings.retention_days}
            min={0}
            onChange={(e) => void updateSetting("retention_days", e.target.value)}
            style={{ maxWidth: "10rem" }}
          />
          <small style={{ display: "block", marginTop: "0.35rem", color: "var(--color-text-muted)", fontSize: "0.8rem" }}>
            {t("settings.retention_days_hint")}
          </small>
        </div>
      </section>
    </section>
  );
}
