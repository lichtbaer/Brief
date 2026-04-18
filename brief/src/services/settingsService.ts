/**
 * settingsService — centralised wrapper around all settings-related Tauri commands.
 *
 * Isolates `invoke("update_setting", ...)` calls in one place so renaming or
 * extending the backend command doesn't require grep-and-edit across every view.
 */

import { invoke } from "@tauri-apps/api/core";
import type { AppSettingsSnapshot, PersistedSettings, SettingDefaults } from "../types";

/** Persists a single setting key/value pair to the database. */
export async function updateSetting(key: keyof PersistedSettings, value: string): Promise<void> {
  await invoke("update_setting", { key, value });
}

/** Returns the current values of all persisted settings. */
export async function getAllSettings(): Promise<PersistedSettings> {
  const raw = await invoke<string>("get_all_settings");
  return JSON.parse(raw) as PersistedSettings;
}

/** Batch-writes a full settings snapshot to the database. */
export async function setAllSettings(settings: Partial<PersistedSettings>): Promise<void> {
  await invoke("set_all_settings", { settings });
}

/** Returns canonical default values for all settings from the Rust backend. */
export async function getSettingDefaults(): Promise<SettingDefaults> {
  return invoke<SettingDefaults>("get_setting_defaults");
}

/** Returns the in-memory app settings snapshot (RAM, model recommendation, onboarding flags). */
export async function getAppSettingsSnapshot(): Promise<AppSettingsSnapshot> {
  return invoke<AppSettingsSnapshot>("get_app_settings_snapshot");
}

/** Returns the list of available CPAL audio input device names. */
export async function listAudioDevices(): Promise<string[]> {
  return invoke<string[]>("list_audio_devices");
}

/** Triggers bulk re-summarization for all meetings (or a specific type). Returns { regenerated, errors }. */
export async function bulkRegenerateMeetings(meetingType?: string): Promise<{ regenerated: number; errors: number }> {
  const raw = await invoke<string>("bulk_regenerate_meetings", {
    meetingType: meetingType ?? null,
  });
  return JSON.parse(raw) as { regenerated: number; errors: number };
}
