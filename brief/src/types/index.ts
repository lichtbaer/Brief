export type MeetingType = "consulting" | "legal" | "internal" | "custom";

/** Must match `transcribe::TRANSCRIPTION_TIMEOUT_ERROR` in the Tauri backend. */
export const TRANSCRIPTION_TIMEOUT_ERROR = "BRIEF_ERR_TRANSCRIPTION_TIMEOUT";

/** Keys persisted in SQLite `settings` (snake_case matches DB). */
export interface PersistedSettings {
  onboarding_complete?: string;
  ollama_url: string;
  llm_model: string;
  default_meeting_type: string;
  meeting_language: string;
  retain_audio: string;
  retention_days: string;
  ui_language?: string;
  /** WhisperX subprocess timeout in seconds (default 900 = 15 min). */
  whisperx_timeout_secs?: string;
  /** Ollama HTTP request timeout in seconds (default 300). */
  ollama_timeout_secs?: string;
  /** Name of the preferred CPAL input device; "default" uses the system default. */
  audio_device?: string;
  /** User-defined system prompt used when meeting_type is "custom". Empty string = use default. */
  custom_prompt_template?: string;
}

/** Mirrors `defaults::SettingDefaults` — canonical defaults from Rust, fetched once at startup. */
export interface SettingDefaults {
  ollama_url: string;
  llm_model: string;
  default_meeting_type: string;
  meeting_language: string;
  retain_audio: string;
  retention_days: string;
  ui_language: string;
  whisperx_timeout_secs: string;
  /** Ollama HTTP request timeout in seconds. */
  ollama_timeout_secs: string;
  processing_step_hint_secs: number;
  /** Current prompt template version from the backend. Used to detect stale analyses. */
  template_version: string;
}

/** Mirrors `AppSettingsSnapshot` from the Tauri backend. */
export interface AppSettingsSnapshot {
  memoryGb: number;
  recommendedModel: string;
  llmModel: string;
  llmModelUserOverride: boolean;
  showLowRamOnboarding: boolean;
}

/** One entry from `check_orphaned_recordings` (temp WAV metadata). */
export interface OrphanedRecording {
  path: string;
  filename: string;
  /** Formatted megabytes, e.g. "4.2". */
  size_mb: string;
}

export interface ActionItem {
  description: string;
  owner: string | null;
  due_date: string | null;
  priority: "high" | "medium" | "low" | null;
}

export interface Topic {
  title: string;
  summary: string;
  duration_estimate?: string;
}

export interface Decision {
  description: string;
  context?: string;
}

/** LLM may return an empty object when follow-up is not generated. */
export interface FollowUpDraft {
  subject?: string;
  greeting?: string;
  body?: string;
  closing?: string;
  full_text?: string;
}

export interface MeetingOutput {
  summary_short: string;
  topics: Topic[];
  decisions: Decision[];
  action_items: ActionItem[];
  follow_up_draft: FollowUpDraft;
  participants_mentioned: string[];
  template_used: string;
  model_used: string;
  generated_at: string;
  /** Prompt template version used to generate this output. Empty for legacy records. */
  template_version?: string;
}

export interface Meeting {
  id: string;
  created_at: string;
  ended_at: string;
  duration_seconds: number;
  meeting_type: MeetingType;
  title: string;
  transcript: string;
  output: MeetingOutput;
  audio_path: string | null;
  tags: string[];
  /** Speaker label → display name mapping (e.g. { "SPEAKER_00": "Alice" }).
   *  Applied at display time only — the stored transcript preserves original labels for FTS. */
  speaker_names?: Record<string, string>;
  /** Diarized segments with timestamps, persisted from WhisperX output.
   *  Absent for meetings recorded before this feature was added. */
  segments?: DiarizedSegment[];
}

/** One diarized utterance; mirrors `transcribe::DiarizedSegment` in the Tauri backend. */
export interface DiarizedSegment {
  speaker: string;
  start: number;
  end: number;
  text: string;
}

/** One cross-meeting action item entry returned by `get_all_action_items`. */
export interface CrossMeetingActionItem {
  meeting_id: string;
  meeting_title: string;
  meeting_created_at: string;
  description: string | null;
  owner: string | null;
  due_date: string | null;
  priority: "high" | "medium" | "low" | null;
}

const MEETING_TYPES: MeetingType[] = ["consulting", "legal", "internal", "custom"];

/**
 * Runtime check for values deserialized from the backend (e.g. invoke / JSON).
 * Narrows `unknown` to Meeting when shape matches.
 */
export function isMeeting(value: unknown): value is Meeting {
  if (value === null || typeof value !== "object") return false;
  const o = value as Record<string, unknown>;
  if (typeof o.id !== "string") return false;
  if (typeof o.created_at !== "string") return false;
  if (typeof o.ended_at !== "string") return false;
  if (typeof o.duration_seconds !== "number") return false;
  if (!MEETING_TYPES.includes(o.meeting_type as MeetingType)) return false;
  if (typeof o.title !== "string") return false;
  if (typeof o.transcript !== "string") return false;
  if (typeof o.audio_path !== "string" && o.audio_path !== null) return false;
  if (!Array.isArray(o.tags) || !o.tags.every((t) => typeof t === "string")) return false;
  if (o.output === null || typeof o.output !== "object") return false;
  const out = o.output as Record<string, unknown>;
  if (typeof out.summary_short !== "string") return false;
  if (!Array.isArray(out.topics)) return false;
  if (!Array.isArray(out.decisions)) return false;
  if (!Array.isArray(out.action_items)) return false;
  if (out.follow_up_draft === null || typeof out.follow_up_draft !== "object") return false;
  if (!Array.isArray(out.participants_mentioned)) return false;
  if (typeof out.template_used !== "string") return false;
  if (typeof out.model_used !== "string") return false;
  if (typeof out.generated_at !== "string") return false;
  return true;
}

/**
 * Runtime check for a WhisperX segment object from JSON.
 */
export function isDiarizedSegment(value: unknown): value is DiarizedSegment {
  if (value === null || typeof value !== "object") return false;
  const o = value as Record<string, unknown>;
  return (
    typeof o.speaker === "string" &&
    typeof o.start === "number" &&
    typeof o.end === "number" &&
    typeof o.text === "string"
  );
}
