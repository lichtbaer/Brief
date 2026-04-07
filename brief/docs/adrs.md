# Architecture decision records (ADRs)

Brief records major product/stack choices as ADRs elsewhere in the organization. The developer docs reference at least:

| ID | Topic |
|----|--------|
| **ADR-009** | Tauri / Rust desktop stack (vs. e.g. Python/FastAPI web stack) — privacy and native desktop requirements. |
| **ADR-010** | WhisperX as the transcription and diarization backend (Ollama no longer ships an official Whisper model as of early 2026). |

For the authoritative ADR text and newer decisions, use your internal knowledge base or the Linear / docs system where ADRs are stored.

| **ADR-011** | FTS5 standalone table with manual backfill (see below). |

---

## ADR-011 — FTS5 standalone table with one-time backfill

**Status:** Accepted  
**Date:** 2026-04-07

### Context

SQLite's FTS5 extension supports an *external-content* mode where the full-text index shadows a regular table and stays in sync via triggers. With SQLCipher's encrypted WAL mode, the trigger-driven sync consistently failed to populate the inverted index on first launch, leaving search returning no results.

### Decision

Use a **standalone FTS5 virtual table** (`meetings_fts`) instead of external-content mode. The table is created once (guarded by a `sqlite_master` check) and backfilled from `meetings` at that point. Subsequent inserts/deletes are kept in sync explicitly inside `save_meeting`, `delete_meeting`, and `update_meeting_title` — each operation updates both `meetings` and `meetings_fts` inside the same transaction, so they never diverge.

### Consequences

- **Good:** Reliable with SQLCipher; no trigger dependency.
- **Good:** Transactional consistency — partial writes are impossible.
- **Bad:** FTS index is not self-healing; a manual `INSERT INTO meetings_fts … SELECT … FROM meetings` migration is required if a future bug creates drift.
- **Neutral:** Slightly more code per mutation (one extra SQL statement per call site).

---

## Known product debt

Track open bugs, UX debt, and technical debt in **Linear** (project Brief). This site intentionally does not duplicate ticket lists.
