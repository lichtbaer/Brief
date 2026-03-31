import { invoke } from "@tauri-apps/api/core";
import type { MouseEvent } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { StatsPanel } from "../components/StatsPanel";
import { isMeeting, type Meeting } from "../types";

export interface MeetingSummary {
  id: string;
  created_at: string;
  meeting_type: string;
  title: string;
  summary_short?: string;
  action_items_count?: number;
  duration_seconds?: number;
  tags?: string[];
}

/** Paginated response shape from the `list_meetings` Tauri command. */
interface ListMeetingsResponse {
  meetings: MeetingSummary[];
  has_more: boolean;
  next_cursor: string | null;
}

interface HistoryViewProps {
  /** Invoked with a full `Meeting` after `get_meeting` when the user opens a list item. */
  onOpenMeeting: (meeting: Meeting) => void;
}

export function formatMeetingDate(iso: string, locale: string): string {
  return new Date(iso).toLocaleDateString(locale, {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Formats a duration in seconds as "Xh Ym" or "Ym Zs" depending on length. */
export function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

function SkeletonCards() {
  return (
    <div aria-hidden="true">
      {[1, 2, 3].map((i) => (
        <div key={i} className="skeleton skeleton-card" />
      ))}
    </div>
  );
}

/**
 * Lists past meetings (newest first) with optional FTS search and tag filtering.
 * Loads meetings in pages of 20 via cursor-based pagination.
 *
 * @param props.onOpenMeeting — parent handles navigation to detail/output.
 */
export function HistoryView({ onOpenMeeting }: HistoryViewProps) {
  const { t, i18n } = useTranslation();
  const [meetings, setMeetings] = useState<MeetingSummary[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [activeTagFilter, setActiveTagFilter] = useState<string | null>(null);
  const [activeTypeFilter, setActiveTypeFilter] = useState<string | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Track whether we are in search mode or tag-filter mode so pagination resets correctly.
  const searchActiveRef = useRef(false);

  const loadMeetings = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    setActiveTagFilter(null);
    setActiveTypeFilter(null);
    searchActiveRef.current = false;
    try {
      const result = await invoke<string>("list_meetings", { before: undefined });
      const { meetings: page, has_more, next_cursor } = JSON.parse(result) as ListMeetingsResponse;
      setMeetings(page);
      setHasMore(has_more);
      setNextCursor(next_cursor);
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadMeetings();
  }, [loadMeetings]);

  /** Appends the next page of results to the existing meeting list. */
  const loadMore = async () => {
    if (!hasMore || loadingMore || !nextCursor) return;
    setLoadingMore(true);
    try {
      const result = await invoke<string>("list_meetings", { before: nextCursor });
      const { meetings: page, has_more, next_cursor } = JSON.parse(result) as ListMeetingsResponse;
      setMeetings((prev) => [...prev, ...page]);
      setHasMore(has_more);
      setNextCursor(next_cursor);
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoadingMore(false);
    }
  };

  /**
   * Fetches and displays meetings matching the search query, tag filter, and type filter.
   * Priority: if a search query is present, FTS is used and tag/type are applied client-side.
   * If only tag or type filter is active, the dedicated backend query is used.
   * Called whenever any filter or search query changes.
   */
  const applyFilters = async (q: string, tag: string | null, type: string | null) => {
    // Minimum search length: single-char queries produce too many low-quality FTS matches.
    const SEARCH_MIN_LENGTH = 2;
    const trimmed = q.trim();

    if (trimmed.length < SEARCH_MIN_LENGTH) {
      if (tag) {
        // Tag-only (or tag+type) filter: use dedicated backend query then filter client-side by type.
        searchActiveRef.current = false;
        setHasMore(false);
        setNextCursor(null);
        setLoading(true);
        setLoadError(null);
        try {
          const result = await invoke<string>("list_meetings_by_tag", { tag });
          let results = JSON.parse(result) as MeetingSummary[];
          if (type) results = results.filter((m) => m.meeting_type === type);
          setMeetings(results);
        } catch (e) {
          setLoadError(e instanceof Error ? e.message : String(e));
        } finally {
          setLoading(false);
        }
      } else if (type) {
        // Type-only filter: use dedicated backend query.
        searchActiveRef.current = false;
        setHasMore(false);
        setNextCursor(null);
        setLoading(true);
        setLoadError(null);
        try {
          const result = await invoke<string>("list_meetings_by_type", { meetingType: type });
          setMeetings(JSON.parse(result) as MeetingSummary[]);
        } catch (e) {
          setLoadError(e instanceof Error ? e.message : String(e));
        } finally {
          setLoading(false);
        }
      } else {
        await loadMeetings();
      }
      return;
    }

    // FTS search mode — pagination disabled (results bounded by backend LIMIT).
    searchActiveRef.current = true;
    setHasMore(false);
    setNextCursor(null);
    setLoading(true);
    setLoadError(null);
    try {
      const result = await invoke<string>("search_meetings", { query: q });
      let results = JSON.parse(result) as MeetingSummary[];
      // Apply tag and type filters client-side on top of FTS results.
      if (tag) results = results.filter((m) => m.tags?.includes(tag));
      if (type) results = results.filter((m) => m.meeting_type === type);
      setMeetings(results);
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleSearch = (q: string) => {
    setSearchQuery(q);
    void applyFilters(q, activeTagFilter, activeTypeFilter);
  };

  /** Toggles a tag filter on/off; combined with any active search query and type filter. */
  const handleTagFilter = (tag: string) => {
    const newTag = activeTagFilter === tag ? null : tag;
    setActiveTagFilter(newTag);
    void applyFilters(searchQuery, newTag, activeTypeFilter);
  };

  /** Clears the tag filter while preserving any active search and type filter. */
  const clearTagFilter = () => {
    setActiveTagFilter(null);
    void applyFilters(searchQuery, null, activeTypeFilter);
  };

  /** Toggles a meeting-type filter on/off; combined with any active search and tag filter. */
  const handleTypeFilter = (type: string) => {
    const newType = activeTypeFilter === type ? null : type;
    setActiveTypeFilter(newType);
    void applyFilters(searchQuery, activeTagFilter, newType);
  };

  /** Clears the type filter while preserving any active search and tag filter. */
  const clearTypeFilter = () => {
    setActiveTypeFilter(null);
    void applyFilters(searchQuery, activeTagFilter, null);
  };

  // Derive the date locale from the current UI language so dates respect the
  // user's language preference instead of always showing German formatting.
  const dateLocale = i18n.language === "en" ? "en-GB" : "de-DE";

  const formatDate = (iso: string) => formatMeetingDate(iso, dateLocale);

  /** Prompts for confirmation then soft-deletes a meeting and removes it from local state. */
  const deleteMeeting = async (id: string, e: React.MouseEvent) => {
    // Stop the card's onClick from also opening the meeting.
    e.stopPropagation();
    if (!window.confirm(t("output.delete_confirm"))) return;
    try {
      await invoke("delete_meeting", { id });
      setMeetings((prev) => prev.filter((m) => m.id !== id));
    } catch {
      setOpenError(t("output.delete_error"));
      setTimeout(() => setOpenError(null), 5000);
    }
  };

  const openMeeting = async (id: string) => {
    setOpenError(null);
    try {
      const json = await invoke<string>("get_meeting", { id });
      const meeting = JSON.parse(json) as unknown;
      if (!isMeeting(meeting)) throw new Error("Invalid meeting data");
      onOpenMeeting(meeting);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setOpenError(message);
      setTimeout(() => setOpenError(null), 5000);
    }
  };

  // Collect all unique tags and meeting types from the currently loaded meetings for filter chips.
  const allTags = [...new Set(meetings.flatMap((m) => m.tags ?? []))];
  const allTypes = [...new Set(meetings.map((m) => m.meeting_type))];

  return (
    <section aria-label={t("nav.history")} style={{ maxWidth: "40rem" }}>
      <h1 style={{ fontSize: "1.4rem", fontWeight: 700, marginBottom: "1rem" }}>
        {t("history.title")}
      </h1>

      {/* Statistics dashboard — collapsed by default, expands on click */}
      <StatsPanel />

      <input
        type="search"
        placeholder={t("history.search_placeholder")}
        value={searchQuery}
        onChange={(e) => handleSearch(e.target.value)}
        className="form-input"
        style={{ maxWidth: "32rem", marginBottom: "0.75rem" }}
        aria-label={t("history.search_placeholder")}
      />

      {/* Tag filter row — always shown when tags exist so users can combine search + tag filter */}
      {allTags.length > 0 && (
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "0.4rem",
            marginBottom: "1rem",
            alignItems: "center",
          }}
        >
          <span style={{ fontSize: "0.75rem", color: "var(--color-text-muted)", marginRight: "0.25rem" }}>
            {t("history.filter_by_tag")}:
          </span>
          {allTags.map((tag) => (
            <button
              key={tag}
              type="button"
              onClick={() => handleTagFilter(tag)}
              style={{
                fontSize: "0.75rem",
                padding: "0.15rem 0.55rem",
                borderRadius: "999px",
                border: "1px solid",
                cursor: "pointer",
                background: activeTagFilter === tag ? "var(--color-accent, #3b82f6)" : "transparent",
                color: activeTagFilter === tag ? "#fff" : "var(--color-text-muted)",
                borderColor: activeTagFilter === tag ? "var(--color-accent, #3b82f6)" : "var(--color-border, #d1d5db)",
              }}
            >
              {tag}
            </button>
          ))}
          {activeTagFilter && (
            <button
              type="button"
              onClick={clearTagFilter}
              style={{
                fontSize: "0.75rem",
                padding: "0.15rem 0.55rem",
                borderRadius: "999px",
                border: "1px solid var(--color-border, #d1d5db)",
                cursor: "pointer",
                background: "transparent",
                color: "var(--color-text-muted)",
              }}
            >
              {t("history.tag_filter_clear")} ×
            </button>
          )}
        </div>
      )}

      {/* Meeting type filter row — shown when more than one type exists */}
      {allTypes.length > 1 && (
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "0.4rem",
            marginBottom: "1rem",
            alignItems: "center",
          }}
        >
          <span style={{ fontSize: "0.75rem", color: "var(--color-text-muted)", marginRight: "0.25rem" }}>
            {t("history.filter_by_type")}:
          </span>
          {allTypes.map((type) => (
            <button
              key={type}
              type="button"
              onClick={() => handleTypeFilter(type)}
              style={{
                fontSize: "0.75rem",
                padding: "0.15rem 0.55rem",
                borderRadius: "999px",
                border: "1px solid",
                cursor: "pointer",
                background: activeTypeFilter === type ? "var(--color-accent, #3b82f6)" : "transparent",
                color: activeTypeFilter === type ? "#fff" : "var(--color-text-muted)",
                borderColor: activeTypeFilter === type ? "var(--color-accent, #3b82f6)" : "var(--color-border, #d1d5db)",
              }}
            >
              {t(`meeting_types.${type}`)}
            </button>
          ))}
          {activeTypeFilter && (
            <button
              type="button"
              onClick={clearTypeFilter}
              style={{
                fontSize: "0.75rem",
                padding: "0.15rem 0.55rem",
                borderRadius: "999px",
                border: "1px solid var(--color-border, #d1d5db)",
                cursor: "pointer",
                background: "transparent",
                color: "var(--color-text-muted)",
              }}
            >
              {t("history.type_filter_clear")} ×
            </button>
          )}
        </div>
      )}

      {loadError && (
        <div className="alert alert-error" role="alert">
          <span>⚠</span>
          <span>{t("errors.alert", { message: loadError })}</span>
        </div>
      )}

      {openError && (
        <div className="alert alert-error">
          <span>⚠</span>
          <span>{t("errors.alert", { message: openError })}</span>
        </div>
      )}

      {loading ? (
        <div aria-busy="true" aria-label={t("history.loading")}>
          <SkeletonCards />
        </div>
      ) : meetings.length === 0 ? (
        <div className="empty-state" role="status">
          <span className="empty-state-icon">🎙</span>
          <p>{t("history.empty_state")}</p>
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column" }}>
          {meetings.map((m) => (
            <button
              key={m.id}
              type="button"
              className="meeting-card"
              onClick={() => void openMeeting(m.id)}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "baseline",
                  gap: "0.5rem",
                  marginBottom: "0.35rem",
                }}
              >
                <span
                  style={{
                    fontSize: "0.75rem",
                    fontWeight: 600,
                    color: "var(--color-text-muted)",
                    textTransform: "uppercase",
                    letterSpacing: "0.04em",
                  }}
                >
                  {t(`meeting_types.${m.meeting_type}`)}
                </span>
                <span style={{ display: "flex", gap: "0.5rem", alignItems: "baseline" }}>
                  {m.duration_seconds !== undefined && (
                    <span style={{ fontSize: "0.75rem", color: "var(--color-text-subtle)" }}>
                      {formatDuration(m.duration_seconds)}
                    </span>
                  )}
                  <span style={{ fontSize: "0.8rem", color: "var(--color-text-subtle)" }}>
                    {formatDate(m.created_at)}
                  </span>
                </span>
              </div>
              <h2 style={{ fontSize: "1rem", fontWeight: 600, margin: "0 0 0.35rem", color: "var(--color-text)" }}>
                {m.title}
              </h2>
              {typeof m.summary_short === "string" && m.summary_short.length > 0 && (
                <p style={{ margin: "0 0 0.5rem", color: "var(--color-text-muted)", fontSize: "0.875rem", lineHeight: 1.5 }}>
                  {m.summary_short}
                </p>
              )}
              {m.action_items_count !== undefined && m.action_items_count > 0 && (
                <span style={{ fontSize: "0.8rem", fontWeight: 600, color: "var(--color-success)" }}>
                  {t("history.action_items_count", { count: m.action_items_count })}
                </span>
              )}
              {/* Tags chips shown on cards for quick visual overview */}
              {m.tags && m.tags.length > 0 && (
                <div style={{ display: "flex", flexWrap: "wrap", gap: "0.3rem", marginTop: "0.5rem" }}>
                  {m.tags.map((tag) => (
                    <span
                      key={tag}
                      style={{
                        fontSize: "0.7rem",
                        padding: "0.1rem 0.45rem",
                        borderRadius: "999px",
                        border: "1px solid var(--color-border, #d1d5db)",
                        color: "var(--color-text-muted)",
                        background: "transparent",
                      }}
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              )}
              {/* Inline delete button — stops card click to avoid opening the meeting */}
              <button
                type="button"
                onClick={(e: MouseEvent) => void deleteMeeting(m.id, e)}
                aria-label={t("history.delete_meeting_aria", { title: m.title })}
                style={{
                  marginTop: "0.5rem",
                  background: "none",
                  border: "none",
                  cursor: "pointer",
                  fontSize: "0.75rem",
                  color: "var(--color-text-subtle)",
                  padding: 0,
                  alignSelf: "flex-start",
                }}
              >
                {t("history.delete_meeting")}
              </button>
            </button>
          ))}

          {/* Search result limit hint — FTS returns at most 20 results per query */}
          {searchActiveRef.current && meetings.length >= 20 && (
            <p
              role="status"
              style={{
                textAlign: "center",
                fontSize: "0.8rem",
                color: "var(--color-text-subtle)",
                marginTop: "0.5rem",
                padding: "0.5rem",
              }}
            >
              {t("history.search_limit_hint")}
            </p>
          )}

          {/* Load more — only shown when not searching and more pages exist */}
          {hasMore && !searchQuery && !activeTagFilter && !activeTypeFilter && (
            <button
              type="button"
              className="btn btn-ghost"
              style={{ marginTop: "0.75rem", alignSelf: "center" }}
              onClick={() => void loadMore()}
              disabled={loadingMore}
            >
              {loadingMore ? (
                <><span className="spinner spinner-dark" />{t("history.loading_more")}</>
              ) : (
                t("history.load_more")
              )}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
