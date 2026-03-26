//! Tauri commands for application settings management.

use crate::error::AppError;
use crate::memory;
use crate::state::AppState;
use crate::types::AppSettingsSnapshot;

/// Returns the canonical setting defaults so the frontend does not duplicate them.
#[tauri::command]
pub fn get_setting_defaults() -> crate::defaults::SettingDefaults {
    crate::defaults::DEFAULTS
}

/// Returns RAM snapshot, recommended LLM model, current model, and low-RAM onboarding flags for the settings UI.
#[tauri::command]
pub async fn get_app_settings_snapshot(
    state: tauri::State<'_, AppState>,
) -> Result<AppSettingsSnapshot, String> {
    let storage = state.storage.lock().await;
    let memory_gb = memory::get_available_memory_gb();
    let recommended_model = memory::recommended_llm_model(memory_gb).to_string();
    let llm_model = storage
        .get_setting("llm_model")
        .await?
        .unwrap_or_else(|| crate::defaults::LLM_MODEL.to_string());
    let llm_model_user_override = storage
        .get_setting("llm_model_user_override")
        .await?
        .unwrap_or_else(|| "0".to_string())
        == "1";
    let dismissed = storage
        .get_setting("low_ram_onboarding_dismissed")
        .await?
        .unwrap_or_else(|| "0".to_string())
        == "1";
    let show_low_ram_onboarding = memory_gb <= 8.0 && !dismissed;
    Ok(AppSettingsSnapshot {
        memory_gb,
        recommended_model,
        llm_model,
        llm_model_user_override,
        show_low_ram_onboarding,
    })
}

/// Persists the LLM model and marks `llm_model_user_override` so auto-recommendations do not overwrite it.
#[tauri::command]
pub async fn set_llm_model(
    model: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return Err(AppError::ValidationError("Model name must not be empty".into()).into());
    }
    let storage = state.storage.lock().await;
    storage.set_setting("llm_model", trimmed).await?;
    storage.set_setting("llm_model_user_override", "1").await?;
    Ok(())
}

/// Persists the user's choice to hide the low-RAM onboarding hint.
#[tauri::command]
pub async fn dismiss_low_ram_onboarding(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let storage = state.storage.lock().await;
    storage
        .set_setting("low_ram_onboarding_dismissed", "1")
        .await?;
    Ok(())
}

/// Get all settings as a JSON object (string values).
#[tauri::command]
pub async fn get_all_settings(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let storage = state.storage.lock().await;
    storage.get_all_settings().await
}

/// Returns whether the user opted to keep meeting WAV files on disk after processing.
#[tauri::command]
pub async fn get_retain_audio(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let storage = state.storage.lock().await;
    let v = storage
        .get_setting("retain_audio")
        .await?
        .unwrap_or_else(|| "false".to_string());
    Ok(v == "true")
}

/// Persists the retain-audio toggle (`true` / `false` string in settings).
#[tauri::command]
pub async fn set_retain_audio(
    value: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let storage = state.storage.lock().await;
    storage
        .set_setting("retain_audio", if value { "true" } else { "false" })
        .await
}

/// Update a single setting (persists immediately).
#[tauri::command]
pub async fn update_setting(
    key: String,
    value: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if key == "llm_model" {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(AppError::ValidationError("Model name must not be empty".into()).into());
        }
        let storage = state.storage.lock().await;
        storage.set_setting("llm_model", trimmed).await?;
        storage.set_setting("llm_model_user_override", "1").await?;
        return Ok(());
    }
    if key == "whisperx_timeout_secs" {
        let trimmed = value.trim();
        let v: u64 = trimmed.parse().map_err(|_| {
            AppError::ValidationError("Timeout must be a positive number (seconds)".into())
        })?;
        if !(60..=86400).contains(&v) {
            return Err(
                AppError::ValidationError("Allowed range: 60 to 86400 seconds".into()).into(),
            );
        }
        let storage = state.storage.lock().await;
        storage
            .set_setting("whisperx_timeout_secs", &v.to_string())
            .await?;
        return Ok(());
    }
    if key == "meeting_language" {
        let trimmed = value.trim().to_lowercase();
        if !matches!(trimmed.as_str(), "de" | "en") {
            return Err(AppError::ValidationError(
                "Meeting language: only \"de\" or \"en\" allowed".into(),
            )
            .into());
        }
        let storage = state.storage.lock().await;
        storage.set_setting("meeting_language", &trimmed).await?;
        return Ok(());
    }
    let storage = state.storage.lock().await;
    storage.set_setting(&key, &value).await
}
