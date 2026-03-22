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
}

/** Mirrors `AppSettingsSnapshot` from the Tauri backend. */
export interface AppSettingsSnapshot {
  memoryGb: number;
  recommendedModel: string;
  llmModel: string;
  llmModelUserOverride: boolean;
  showLowRamOnboarding: boolean;
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
}
