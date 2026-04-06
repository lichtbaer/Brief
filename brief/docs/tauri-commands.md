# Tauri commands

The frontend calls these handlers via `invoke()`. Registration order is defined in `brief/src-tauri/src/lib.rs` (`tauri::generate_handler![...]`).

Unless noted, async commands return `Result<..., String>` on failure. Many meeting-related endpoints return **JSON strings** for compatibility with the existing TS layer.

## Recording (`commands/recording.rs`)

| Command | Summary |
|---------|---------|
| `start_recording` | Start capture; returns new `session_id` (`meeting_type: String`). |
| `stop_recording` | Stop session; returns temp WAV path (`session_id: String`). |
| `process_meeting` | Run WhisperX + summarization; persists meeting (`session_id`, `audio_path`, `meeting_type`, optional `title_override`). Returns meeting JSON string. |
| `check_orphaned_recordings` | Returns metadata for at most one orphaned temp WAV (recovery banner). |
| `recover_orphaned_recording` | Transcribe orphaned WAV into a new meeting (`audio_path`). |
| `discard_orphaned_recording` | Delete orphaned temp WAV after user confirmation (`audio_path`). |
| `regenerate_summary` | Re-run Ollama on stored transcript (`id`, `meeting_type`). Returns updated meeting JSON. |

## Meetings (`commands/meetings.rs`)

| Command | Summary |
|---------|---------|
| `get_meeting` | Load full meeting JSON by `id`. |
| `list_meetings` | Paginated summaries (`before` cursor optional). |
| `search_meetings` | Full-text search (`query`). |
| `update_meeting_tags` | Replace tags (`id`, `tags: Vec<String>`). |
| `delete_meeting` | Soft-delete meeting (`id`). |
| `update_meeting_title` | Rename (`id`, `title`). |
| `list_meetings_by_type` | Filter by meeting type string. |
| `list_meetings_by_tag` | Filter by tag. |
| `enforce_audio_retention` | Purge expired WAVs; returns count. |
| `update_follow_up_draft` | Patch follow-up draft text (`id`, `text`). |
| `get_meeting_stats` | Aggregated stats JSON. |
| `delete_meetings_before` | Bulk soft-delete before ISO timestamp. |
| `list_meetings_by_date_range` | Date range query (`from_date`, `to_date` as `YYYY-MM-DD`). |
| `get_all_action_items` | Flat action-item list with provenance. |
| `list_meetings_by_participant` | Filter by participant name. |
| `bulk_regenerate_meetings` | Re-summarize many meetings (`meeting_type: Option<String>`). |
| `update_speaker_names` | Persist display-name map (`id`, `names: HashMap<String,String>`). |

## Export (`commands/export.rs`)

| Command | Summary |
|---------|---------|
| `export_markdown` | Markdown for meeting `id` (localized section headers). |
| `export_pdf` | PDF as base64-encoded bytes string for `id`. |
| `get_audio_path` | Resolved WAV path on disk for `id`. |
| `export_audio` | Save dialog copy of WAV (`id`). |
| `export_action_items_csv` | CSV export via save dialog (`id`). |

## Settings (`commands/settings.rs`)

| Command | Summary |
|---------|---------|
| `get_setting_defaults` | Returns `SettingDefaults` (sync). |
| `get_app_settings_snapshot` | RAM, recommended LLM, current model, onboarding flags. |
| `set_llm_model` | Set model and mark user override. |
| `dismiss_low_ram_onboarding` | Hide low-RAM hint. |
| `get_all_settings` | All settings as JSON string. |
| `get_retain_audio` / `set_retain_audio` | WAV retention toggle. |
| `update_setting` | Generic key/value update with validation for known keys (`llm_model`, timeouts, `meeting_language`, `ollama_url`, `custom_prompt_template`, …). |

## Health (`commands/health.rs`)

| Command | Summary |
|---------|---------|
| `check_whisperx` | WhisperX / runner availability. |
| `check_ollama` | Ollama reachability / metadata JSON. |

## Audio devices (`commands/audio.rs`)

| Command | Summary |
|---------|---------|
| `list_audio_devices` | Input device names. |
| `get_audio_level` | RMS level in 0.0–1.0 and buffer-overflow flag for active `session_id` (polled ~5 Hz while recording). |

!!! tip "Source of truth"
    When adding a command, register it in `lib.rs` and extend this table (or generate it from a script in a follow-up).
