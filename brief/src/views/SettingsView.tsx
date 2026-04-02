import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import i18n from "../i18n";
import type { AppSettingsSnapshot, PersistedSettings, SettingDefaults } from "../types";

/** Result shape returned by the `bulk_regenerate_meetings` Tauri command. */
interface BulkRegenResult {
  regenerated: number;
  errors: number;
}

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
    audio_device: raw.audio_device ?? "default",
    custom_prompt_template: raw.custom_prompt_template ?? "",
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
  // Audio device list — loaded once on mount; empty if CPAL enumeration fails.
  const [audioDevices, setAudioDevices] = useState<string[]>([]);
  // Bulk re-summarization state (Feature 5).
  const [bulkRegenRunning, setBulkRegenRunning] = useState(false);
  const [bulkRegenResult, setBulkRegenResult] = useState<BulkRegenResult | null>(null);
  const [bulkRegenMeetingType, setBulkRegenMeetingType] = useState("");

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
    // Load available audio input devices in parallel; non-fatal if it fails.
    void invoke<string[]>("list_audio_devices")
      .then(setAudioDevices)
      .catch(() => setAudioDevices([]));
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

        {/* Custom prompt template — shown when the default meeting type is "custom" */}
        <div className="form-group">
          <label className="form-label" htmlFor="custom-prompt-template">
            {t("settings.custom_prompt_label")}
          </label>
          <textarea
            id="custom-prompt-template"
            className="form-input"
            value={settings.custom_prompt_template ?? ""}
            placeholder={t("settings.custom_prompt_placeholder")}
            onChange={(e) => void updateSetting("custom_prompt_template", e.target.value)}
            style={{ width: "100%", minHeight: "10rem", resize: "vertical", fontSize: "0.85rem", fontFamily: "monospace" }}
          />
          <small style={{ display: "block", marginTop: "0.35rem", color: "var(--color-text-muted)", fontSize: "0.8rem" }}>
            {t("settings.custom_prompt_hint")}
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

        {/* Audio device selector — only shown when CPAL can enumerate devices */}
        {audioDevices.length > 0 && (
          <div className="form-group">
            <label className="form-label" htmlFor="audio-device">
              {t("settings.audio_device")}
            </label>
            <select
              id="audio-device"
              className="form-select"
              value={settings.audio_device ?? "default"}
              onChange={(e) => void updateSetting("audio_device", e.target.value)}
              style={{ maxWidth: "28rem" }}
            >
              <option value="default">{t("settings.audio_device_default")}</option>
              {audioDevices.map((name) => (
                <option key={name} value={name}>{name}</option>
              ))}
            </select>
            <small style={{ display: "block", marginTop: "0.35rem", color: "var(--color-text-muted)", fontSize: "0.8rem" }}>
              {t("settings.audio_device_hint")}
            </small>
          </div>
        )}

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
            <option value="custom">{t("meeting_types.custom")}</option>
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

      {/* Batch actions section — allows re-running Ollama on all stored meetings (Feature 5) */}
      <section style={{ marginTop: "2rem" }}>
        <h2 style={{ fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--color-text-subtle)", fontWeight: 600, marginBottom: "1rem" }}>
          {t("settings.bulk_regenerate_section")}
        </h2>

        <div className="form-group">
          <label className="form-label" htmlFor="bulk-regen-type">
            {t("settings.bulk_regenerate_type_label")}
          </label>
          <select
            id="bulk-regen-type"
            className="form-select"
            value={bulkRegenMeetingType}
            onChange={(e) => setBulkRegenMeetingType(e.target.value)}
            style={{ maxWidth: "18rem" }}
            disabled={bulkRegenRunning}
          >
            <option value="">{t("meeting_types.consulting")} + {t("meeting_types.legal")} + {t("meeting_types.internal")} + {t("meeting_types.custom")}</option>
            <option value="consulting">{t("meeting_types.consulting")}</option>
            <option value="legal">{t("meeting_types.legal")}</option>
            <option value="internal">{t("meeting_types.internal")}</option>
            <option value="custom">{t("meeting_types.custom")}</option>
          </select>
        </div>

        <div className="form-group">
          <p style={{ marginBottom: "0.5rem", fontSize: "0.8rem", color: "var(--color-text-muted)" }}>
            {t("settings.bulk_regenerate_hint")}
          </p>
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            disabled={bulkRegenRunning}
            onClick={() => {
              setBulkRegenResult(null);
              setBulkRegenRunning(true);
              void invoke<string>("bulk_regenerate_meetings", {
                meetingType: bulkRegenMeetingType || null,
              })
                .then((raw) => {
                  setBulkRegenResult(JSON.parse(raw) as BulkRegenResult);
                })
                .catch((e: unknown) => {
                  setSettingsError(String(e));
                  setTimeout(() => setSettingsError(null), 5000);
                })
                .finally(() => setBulkRegenRunning(false));
            }}
          >
            {bulkRegenRunning ? (
              <><span className="spinner spinner-dark" />{t("settings.bulk_regenerate_running")}</>
            ) : (
              t("settings.bulk_regenerate_btn")
            )}
          </button>
          {bulkRegenResult !== null && (
            <p
              role="status"
              style={{ marginTop: "0.5rem", fontSize: "0.85rem", color: "var(--color-text-muted)" }}
            >
              {t("settings.bulk_regenerate_done", {
                count: bulkRegenResult.regenerated,
                errors: bulkRegenResult.errors,
              })}
            </p>
          )}
        </div>
      </section>
    </section>
  );
}
