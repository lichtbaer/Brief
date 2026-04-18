import type { ReactNode, RefObject } from "react";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { DiarizedSegment, MeetingOutput } from "../types";

interface TranscriptSectionProps {
  /** Raw transcript text as stored in the database. */
  transcript: string;
  /** Diarized segments with timestamps; absent for older recordings. */
  segments?: DiarizedSegment[];
  /** AI output — used for participants list. */
  output: MeetingOutput;
  /** Speaker label → display name mapping; applied at render time only. */
  speakerNames: Record<string, string>;
  /** Active playback segment index driven by the audio timeupdate event. */
  currentSegmentIndex: number | null;
  /** Whether retained audio is loaded — controls seek-click cursor/disabled state. */
  audioUrl: string | null;
  /** Ref to the <audio> element for programmatic seeking on segment click. */
  audioRef: RefObject<HTMLAudioElement | null>;
  /** Optional search query; highlights matching terms in transcript text. */
  searchQuery?: string;
  /** Whether the speaker save failed — shown as inline error banner. */
  speakerSaveError: boolean;
  onSpeakerNameChange: (id: string, value: string) => void;
  onSpeakerNameBlur: () => void;
  /** Optional callback when a participant name is clicked (navigates to filtered history). */
  onFilterByParticipant?: (name: string) => void;
}

/**
 * Splits `text` on case-insensitive occurrences of `query` and wraps matches in <mark>.
 * Returns plain text when `query` is empty to avoid unnecessary React reconciliation.
 */
function highlightTerms(text: string, query: string): ReactNode {
  if (!query.trim()) return text;
  // Truncate before escaping to prevent ReDoS on very long queries with special characters.
  const safeQuery = query.slice(0, 200);
  const escaped = safeQuery.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const parts = text.split(new RegExp(`(${escaped})`, "gi"));
  return parts.map((part, i) =>
    i % 2 === 1 ? <mark key={i}>{part}</mark> : part
  );
}

/** Extracts unique speaker label IDs from a raw transcript string (e.g. "SPEAKER_00"). */
function extractSpeakers(transcript: string): string[] {
  const matches = [...transcript.matchAll(/\[([A-Z_0-9]+)\]/g)];
  return [...new Set(matches.map((m) => m[1]))];
}

/**
 * Replaces speaker label brackets with user-defined names (view-only, never modifies DB).
 * Uses global regex replace for ES2020 compatibility.
 */
function applyNames(transcript: string, names: Record<string, string>): string {
  return Object.entries(names).reduce((t, [id, name]) => {
    if (!name.trim()) return t;
    const escaped = id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return t.replace(new RegExp(`\\[${escaped}\\]`, "g"), `[${name.trim()}]`);
  }, transcript);
}

/**
 * TranscriptSection — renders the diarized transcript with click-to-seek support,
 * speaker renaming panel, and participant filter buttons.
 *
 * Extracted from OutputView to reduce its size and allow independent testing.
 */
export function TranscriptSection({
  transcript,
  segments,
  output,
  speakerNames,
  currentSegmentIndex,
  audioUrl,
  audioRef,
  searchQuery,
  speakerSaveError,
  onSpeakerNameChange,
  onSpeakerNameBlur,
  onFilterByParticipant,
}: TranscriptSectionProps) {
  const { t } = useTranslation();

  // Derive speaker list once per render — extractSpeakers is pure but regex-heavy.
  const speakers = useMemo(() => extractSpeakers(transcript), [transcript]);

  // Memoize speaker-name-applied transcript for the plain-text fallback path.
  const namedTranscript = useMemo(
    () => applyNames(transcript, speakerNames),
    [transcript, speakerNames],
  );

  return (
    <>
      {/* Speaker naming panel — only shown when the transcript has diarized speaker labels */}
      {speakers.length > 0 && (
        <section className="output-section">
          <h2>{t("output.speaker_names_title")}</h2>
          <p style={{ fontSize: "0.8rem", color: "var(--color-text-muted)", marginTop: "0.25rem", marginBottom: "0.75rem" }}>
            {t("output.speaker_names_hint")}
          </p>
          {speakerSaveError && (
            <div className="alert alert-error" role="alert" style={{ marginBottom: "0.75rem", fontSize: "0.85rem" }}>
              <span>⚠</span>
              <span>{t("output.speaker_names_save_error")}</span>
            </div>
          )}
          <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
            {speakers.map((id) => (
              <div key={id} style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
                <span style={{ fontSize: "0.8rem", fontFamily: "monospace", color: "var(--color-text-muted)", minWidth: "8rem" }}>
                  {id}
                </span>
                <input
                  type="text"
                  value={speakerNames[id] ?? ""}
                  placeholder={t("output.speaker_name_placeholder")}
                  maxLength={50}
                  onChange={(e) => onSpeakerNameChange(id, e.target.value.slice(0, 50))}
                  onBlur={onSpeakerNameBlur}
                  className="form-input"
                  style={{ maxWidth: "16rem", fontSize: "0.875rem" }}
                  aria-label={`${id} ${t("output.speaker_name_placeholder")}`}
                />
              </div>
            ))}
          </div>
        </section>
      )}

      <section className="output-section">
        {/* Auto-open the details element when the user arrived via a search query (Feature 6) */}
        <details open={!!searchQuery}>
          <summary style={{ cursor: "pointer", userSelect: "none", fontSize: "0.8rem", textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--color-text-subtle)", fontWeight: 600 }}>
            {t("output.transcript")}
          </summary>
          {/* When diarized segments are available, render them with timestamps for click-to-seek.
              Fall back to the plain transcript for older meetings without segment data. */}
          {segments && segments.length > 0 ? (
            <div style={{ marginTop: "0.5rem", display: "flex", flexDirection: "column", gap: "0.4rem" }}>
              {segments.map((seg, i) => {
                const displaySpeaker = speakerNames[seg.speaker] || seg.speaker;
                const startSec = Math.floor(seg.start);
                const mins = String(Math.floor(startSec / 60)).padStart(2, "0");
                const secs = String(startSec % 60).padStart(2, "0");
                const isActive = currentSegmentIndex === i;
                return (
                  // Wrap each segment row in a button so clicking it seeks the audio player (Feature 2).
                  <button
                    key={i}
                    type="button"
                    disabled={!audioUrl}
                    aria-label={t("output.seek_to_segment_aria", { time: `${mins}:${secs}` })}
                    onClick={() => {
                      const audio = audioRef.current;
                      if (!audio) return;
                      audio.currentTime = seg.start;
                      void audio.play();
                    }}
                    style={{
                      display: "flex",
                      gap: "0.75rem",
                      alignItems: "flex-start",
                      fontSize: "0.875rem",
                      lineHeight: 1.5,
                      background: isActive ? "var(--color-accent-subtle, rgba(59,130,246,0.08))" : "none",
                      border: "none",
                      borderLeft: isActive ? "3px solid var(--color-accent, #3b82f6)" : "3px solid transparent",
                      borderRadius: 0,
                      cursor: audioUrl ? "pointer" : "default",
                      padding: "0.15rem 0.25rem",
                      textAlign: "left",
                      width: "100%",
                    }}
                  >
                    <span style={{ fontFamily: "monospace", color: "var(--color-text-subtle)", minWidth: "3.5rem", flexShrink: 0, paddingTop: "0.05rem" }}>
                      {mins}:{secs}
                    </span>
                    <span style={{ fontFamily: "monospace", color: "var(--color-text-muted)", minWidth: "6.5rem", flexShrink: 0, paddingTop: "0.05rem" }}>
                      {displaySpeaker}
                    </span>
                    <span style={{ flex: 1 }}>
                      {searchQuery ? highlightTerms(seg.text, searchQuery) : seg.text}
                    </span>
                  </button>
                );
              })}
            </div>
          ) : (
            <pre className="transcript" style={{ marginTop: "0.5rem", fontFamily: "inherit" }}>
              {searchQuery ? highlightTerms(namedTranscript, searchQuery) : namedTranscript}
            </pre>
          )}
        </details>
      </section>

      {output.participants_mentioned.length > 0 && (
        <section className="output-section">
          <h2>{t("output.participants")}</h2>
          {/* Participant names as clickable buttons navigate to filtered history (Feature 7). */}
          {onFilterByParticipant ? (
            <>
              <p style={{ marginTop: "0.25rem", marginBottom: "0.35rem", fontSize: "0.8rem", color: "var(--color-text-muted)" }}>
                {t("output.participants_filter_hint")}
              </p>
              <div style={{ display: "flex", flexWrap: "wrap", gap: "0.4rem" }}>
                {output.participants_mentioned.map((name) => (
                  <button
                    key={name}
                    type="button"
                    onClick={() => onFilterByParticipant(name)}
                    aria-label={t("output.participants_filter_hint") + " " + name}
                    style={{
                      fontSize: "0.875rem",
                      padding: "0.2rem 0.6rem",
                      borderRadius: "999px",
                      border: "1px solid var(--color-accent, #3b82f6)",
                      background: "transparent",
                      color: "var(--color-accent, #3b82f6)",
                      cursor: "pointer",
                    }}
                  >
                    {name}
                  </button>
                ))}
              </div>
            </>
          ) : (
            <p style={{ marginTop: "0.25rem" }}>
              {output.participants_mentioned.join(", ")}
            </p>
          )}
        </section>
      )}
    </>
  );
}
