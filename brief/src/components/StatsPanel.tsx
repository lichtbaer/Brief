/**
 * StatsPanel — meeting statistics dashboard embedded in HistoryView.
 * Displays total meetings, cumulative duration, type breakdown, action item count,
 * and a 12-week sparkline. All data is fetched from the local encrypted DB via
 * the `get_meeting_stats` Tauri command. No external requests are made.
 */
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

interface MeetingStats {
  total_meetings: number;
  total_seconds: number;
  by_type: Array<{ type: string; count: number }>;
  total_action_items: number;
  weekly: Array<{ week: string; count: number }>;
}

/** Formats a duration in seconds as "Xh Ym" or "Ym" when under 1 hour. */
function formatDurationHours(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/**
 * Renders a compact SVG sparkline for weekly meeting counts.
 * Uses a simple polyline — no charting library to keep the bundle lean.
 */
function Sparkline({ data }: { data: Array<{ week: string; count: number }> }) {
  if (data.length < 2) return null;

  const W = 120;
  const H = 28;
  const maxCount = Math.max(...data.map((d) => d.count), 1);

  const points = data
    .map((d, i) => {
      const x = (i / (data.length - 1)) * W;
      const y = H - (d.count / maxCount) * (H - 4);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");

  return (
    <svg
      width={W}
      height={H}
      aria-hidden="true"
      style={{ display: "block", overflow: "visible" }}
    >
      <polyline
        points={points}
        fill="none"
        stroke="var(--color-primary, #2563eb)"
        strokeWidth="1.5"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function StatsPanel() {
  const { t } = useTranslation();
  const [stats, setStats] = useState<MeetingStats | null>(null);

  useEffect(() => {
    void invoke<string>("get_meeting_stats")
      .then((json) => setStats(JSON.parse(json) as MeetingStats))
      .catch(() => setStats(null));
  }, []);

  if (!stats || stats.total_meetings === 0) return null;

  return (
    <details style={{ marginBottom: "1.5rem" }}>
      <summary
        style={{
          cursor: "pointer",
          userSelect: "none",
          fontSize: "0.8rem",
          textTransform: "uppercase",
          letterSpacing: "0.07em",
          color: "var(--color-text-subtle)",
          fontWeight: 600,
          marginBottom: "0.75rem",
        }}
      >
        {t("stats.title")}
      </summary>

      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          gap: "1.25rem",
          padding: "0.75rem 0",
          alignItems: "flex-start",
        }}
      >
        {/* Total meetings */}
        <div>
          <div style={{ fontSize: "1.5rem", fontWeight: 700, lineHeight: 1.1 }}>
            {stats.total_meetings}
          </div>
          <div style={{ fontSize: "0.78rem", color: "var(--color-text-muted)", marginTop: "0.15rem" }}>
            {t("stats.total_meetings")}
          </div>
        </div>

        {/* Total duration */}
        <div>
          <div style={{ fontSize: "1.5rem", fontWeight: 700, lineHeight: 1.1 }}>
            {formatDurationHours(stats.total_seconds)}
          </div>
          <div style={{ fontSize: "0.78rem", color: "var(--color-text-muted)", marginTop: "0.15rem" }}>
            {t("stats.total_duration")}
          </div>
        </div>

        {/* Action items */}
        <div>
          <div style={{ fontSize: "1.5rem", fontWeight: 700, lineHeight: 1.1 }}>
            {stats.total_action_items}
          </div>
          <div style={{ fontSize: "0.78rem", color: "var(--color-text-muted)", marginTop: "0.15rem" }}>
            {t("stats.total_action_items")}
          </div>
        </div>

        {/* Type breakdown — inline bar segments */}
        {stats.by_type.length > 1 && (
          <div>
            <div style={{ display: "flex", gap: "0.35rem", height: "12px", width: "80px", borderRadius: "4px", overflow: "hidden", marginBottom: "0.35rem" }}>
              {stats.by_type.map((item) => (
                <div
                  key={item.type}
                  title={`${item.type}: ${item.count}`}
                  style={{
                    flex: item.count,
                    background: "var(--color-primary, #2563eb)",
                    opacity: 0.3 + (stats.by_type.indexOf(item) === 0 ? 0.5 : 0),
                  }}
                />
              ))}
            </div>
            <div style={{ fontSize: "0.78rem", color: "var(--color-text-muted)" }}>
              {t("stats.by_type")}
            </div>
          </div>
        )}

        {/* Weekly trend sparkline */}
        {stats.weekly.length >= 2 && (
          <div>
            <Sparkline data={stats.weekly} />
            <div style={{ fontSize: "0.78rem", color: "var(--color-text-muted)", marginTop: "0.25rem" }}>
              {t("stats.weekly_trend")}
            </div>
          </div>
        )}
      </div>
    </details>
  );
}
