/**
 * meetingService — centralised wrapper around all meeting-related Tauri commands.
 *
 * Keeps `invoke()` calls out of view components so that:
 *   - command names live in one place (easy to refactor),
 *   - JSON parsing happens here rather than at each call site,
 *   - type narrowing is applied once and propagated via return types.
 *
 * Functions throw the backend error string on failure (same as Tauri's Result<_, String>).
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  CrossMeetingActionItem,
  ListMeetingsResponse,
  Meeting,
  MeetingOutput,
  MeetingSummary,
} from "../types";

/** Returns the first page of meetings (newest first). Pass `cursor` for subsequent pages. */
export async function listMeetings(cursor?: string): Promise<ListMeetingsResponse> {
  const raw = await invoke<string>("list_meetings", { before: cursor ?? undefined });
  return JSON.parse(raw) as ListMeetingsResponse;
}

/** Returns FTS search results for `query`. */
export async function searchMeetings(query: string): Promise<MeetingSummary[]> {
  const raw = await invoke<string>("search_meetings", { query });
  return JSON.parse(raw) as MeetingSummary[];
}

/** Returns meetings within a date range (inclusive, ISO date strings: "YYYY-MM-DD"). */
export async function listMeetingsByDateRange(
  from: string,
  to: string,
): Promise<MeetingSummary[]> {
  const raw = await invoke<string>("list_meetings_by_date_range", {
    fromDate: from,
    toDate: to,
  });
  return JSON.parse(raw) as MeetingSummary[];
}

/** Returns meetings that mention `participant` in their transcript. */
export async function listMeetingsByParticipant(participant: string): Promise<MeetingSummary[]> {
  const raw = await invoke<string>("list_meetings_by_participant", { name: participant });
  return JSON.parse(raw) as MeetingSummary[];
}

/** Returns meetings tagged with `tag`. */
export async function listMeetingsByTag(tag: string): Promise<MeetingSummary[]> {
  const raw = await invoke<string>("list_meetings_by_tag", { tag });
  return JSON.parse(raw) as MeetingSummary[];
}

/** Returns meetings of the given meeting type. */
export async function listMeetingsByType(type: string): Promise<MeetingSummary[]> {
  const raw = await invoke<string>("list_meetings_by_type", { meetingType: type });
  return JSON.parse(raw) as MeetingSummary[];
}

/** Loads the full meeting record by id. Returns null when not found. */
export async function getMeeting(id: string): Promise<Meeting | null> {
  const raw = await invoke<string>("get_meeting", { id });
  if (!raw) return null;
  return JSON.parse(raw) as Meeting;
}

/** Soft-deletes a meeting by id. */
export async function deleteMeeting(id: string): Promise<void> {
  await invoke("delete_meeting", { id });
}

/** Bulk soft-deletes all meetings created before `before` (ISO timestamp). Returns count deleted. */
export async function deleteMeetingsBefore(before: string): Promise<number> {
  return invoke<number>("delete_meetings_before", { before });
}

/** Updates the display title of a meeting. */
export async function updateMeetingTitle(id: string, title: string): Promise<void> {
  await invoke("update_meeting_title", { id, title });
}

/** Replaces the tag list for a meeting. */
export async function updateMeetingTags(id: string, tags: string[]): Promise<void> {
  await invoke("update_meeting_tags", { id, tags });
}

/** Persists speaker label → display name mapping. The stored transcript is not modified. */
export async function updateSpeakerNames(
  id: string,
  names: Record<string, string>,
): Promise<void> {
  await invoke("update_speaker_names", { id, names });
}

/** Patches the follow-up email draft text inside the stored output JSON. */
export async function updateFollowUpDraft(id: string, text: string): Promise<void> {
  await invoke("update_follow_up_draft", { id, text });
}

/** Re-runs the LLM summarizer for a single meeting and returns the updated MeetingOutput. */
export async function regenerateSummary(
  id: string,
  meetingType: string,
): Promise<MeetingOutput> {
  const raw = await invoke<string>("regenerate_summary", { id, meetingType });
  return JSON.parse(raw) as MeetingOutput;
}

/** Retrieves the resolved filesystem path for the retained audio file of a meeting. */
export async function getAudioPath(id: string): Promise<string> {
  return invoke<string>("get_audio_path", { id });
}

/** Returns all open action items across all meetings, sorted by due date. */
export async function getAllActionItems(): Promise<CrossMeetingActionItem[]> {
  const raw = await invoke<string>("get_all_action_items");
  return JSON.parse(raw) as CrossMeetingActionItem[];
}
