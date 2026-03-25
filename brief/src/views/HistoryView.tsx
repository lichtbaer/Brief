import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { isMeeting, type Meeting } from "../types";

export interface MeetingSummary {
  id: string;
  created_at: string;
  meeting_type: string;
  title: string;
  summary_short?: string;
  action_items_count?: number;
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
 * Lists past meetings (newest first) with optional FTS search; loads full meeting JSON on card click.
 *
 * @param props.onOpenMeeting — parent handles navigation to detail/output.
 */
export function HistoryView({ onOpenMeeting }: HistoryViewProps) {
  const { t, i18n } = useTranslation();
  const [meetings, setMeetings] = useState<MeetingSummary[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [openError, setOpenError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const loadMeetings = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const result = await invoke<string>("list_meetings");
      setMeetings(JSON.parse(result) as MeetingSummary[]);
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadMeetings();
  }, [loadMeetings]);

  const handleSearch = async (q: string) => {
    setSearchQuery(q);
    const trimmed = q.trim();
    if (trimmed.length < 2) {
      await loadMeetings();
      return;
    }
    setLoading(true);
    setLoadError(null);
    try {
      const result = await invoke<string>("search_meetings", { query: q });
      setMeetings(JSON.parse(result) as MeetingSummary[]);
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  // Derive the date locale from the current UI language so dates respect the
  // user's language preference instead of always showing German formatting.
  const dateLocale = i18n.language === "en" ? "en-GB" : "de-DE";

  const formatDate = (iso: string) => formatMeetingDate(iso, dateLocale);

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

  return (
    <section aria-label={t("nav.history")} style={{ maxWidth: "40rem" }}>
      <h1 style={{ fontSize: "1.4rem", fontWeight: 700, marginBottom: "1rem" }}>
        {t("history.title")}
      </h1>

      <input
        type="search"
        placeholder={t("history.search_placeholder")}
        value={searchQuery}
        onChange={(e) => void handleSearch(e.target.value)}
        className="form-input"
        style={{ maxWidth: "32rem", marginBottom: "1.25rem" }}
        aria-label={t("history.search_placeholder")}
      />

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
                <span style={{ fontSize: "0.8rem", color: "var(--color-text-subtle)" }}>
                  {formatDate(m.created_at)}
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
            </button>
          ))}
        </div>
      )}
    </section>
  );
}
