export type MeetingType = "consulting" | "legal" | "internal" | "custom";

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

export interface FollowUpDraft {
  subject: string;
  greeting: string;
  body: string;
  closing: string;
  full_text: string;
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
