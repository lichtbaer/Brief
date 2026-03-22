import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Meeting } from "../types";

export interface MeetingSummary {
  id: string;
  created_at: string;
  meeting_type: string;
  title: string;
  summary_short?: string;
  action_items_count?: number;
}

interface HistoryViewProps {
  onOpenMeeting: (meeting: Meeting) => void;
}

export function HistoryView({ onOpenMeeting }: HistoryViewProps) {
  const { t } = useTranslation();
  const [meetings, setMeetings] = useState<MeetingSummary[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [loading, setLoading] = useState(true);

  const loadMeetings = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<string>("list_meetings");
      setMeetings(JSON.parse(result) as MeetingSummary[]);
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
    try {
      const result = await invoke<string>("search_meetings", { query: q });
      setMeetings(JSON.parse(result) as MeetingSummary[]);
    } finally {
      setLoading(false);
    }
  };

  const formatDate = (iso: string) =>
    new Date(iso).toLocaleDateString("de-DE", {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });

  const openMeeting = async (id: string) => {
    try {
      const json = await invoke<string>("get_meeting", { id });
      const meeting = JSON.parse(json) as Meeting;
      onOpenMeeting(meeting);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      window.alert(t("errors.alert", { message }));
    }
  };

  return (
    <section aria-label={t("nav.history")} className="history-view">
      <h1 style={{ fontSize: "1.25rem", marginBottom: "0.75rem" }}>
        {t("history.title")}
      </h1>

      <input
        type="search"
        placeholder={t("history.search_placeholder")}
        value={searchQuery}
        onChange={(e) => void handleSearch(e.target.value)}
        className="search-input"
        style={{
          width: "100%",
          maxWidth: "32rem",
          padding: "0.5rem 0.75rem",
          marginBottom: "1rem",
          borderRadius: "6px",
          border: "1px solid #e5e5e5",
          fontSize: "1rem",
        }}
        aria-label={t("history.search_placeholder")}
      />

      {loading ? (
        <p role="status">{t("history.loading")}</p>
      ) : meetings.length === 0 ? (
        <p role="status">{t("history.empty")}</p>
      ) : (
        <div className="meeting-list" style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
          {meetings.map((m) => (
            <div
              key={m.id}
              role="button"
              tabIndex={0}
              className="meeting-card"
              onClick={() => void openMeeting(m.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  void openMeeting(m.id);
                }
              }}
              style={{
                padding: "0.75rem 1rem",
                border: "1px solid #e5e5e5",
                borderRadius: "8px",
                cursor: "pointer",
                background: "#fafafa",
              }}
            >
              <div
                className="meeting-card-header"
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "baseline",
                  gap: "0.5rem",
                  marginBottom: "0.35rem",
                }}
              >
                <span
                  className="meeting-type-badge"
                  style={{
                    fontSize: "0.75rem",
                    fontWeight: 600,
                    color: "#525252",
                  }}
                >
                  {t(`meeting_types.${m.meeting_type}`)}
                </span>
                <span className="meeting-date" style={{ fontSize: "0.8rem", color: "#737373" }}>
                  {formatDate(m.created_at)}
                </span>
              </div>
              <h2 style={{ fontSize: "1.05rem", margin: "0 0 0.35rem" }}>{m.title}</h2>
              {typeof m.summary_short === "string" && m.summary_short.length > 0 && (
                <p className="summary-preview" style={{ margin: "0 0 0.5rem", color: "#404040", fontSize: "0.9rem" }}>
                  {m.summary_short}
                </p>
              )}
              {m.action_items_count !== undefined && m.action_items_count > 0 && (
                <span
                  className="action-items-badge"
                  style={{ fontSize: "0.8rem", fontWeight: 600, color: "#166534" }}
                >
                  {t("history.action_items_count", { count: m.action_items_count })}
                </span>
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
