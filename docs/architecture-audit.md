# Architektur-Audit & Maßnahmenplan

**Datum:** 2026-04-07  
**Scope:** Vollständige Inspektion von Frontend (React/TypeScript), Backend (Rust/Tauri), Python-Subprocess (WhisperX) und CI/CD

---

## 1. Gesamtbefund

| Kategorie | Bewertung | Kernbefund |
|---|---|---|
| Projektstruktur | ✅ Sehr gut | Klare Trennung Frontend / Backend / Python; MkDocs-Dokumentation vorhanden |
| Architekturmuster | ✅ Stark | Layered Design, modulare Tauri-Commands, eigene Fehlertypen, RAII-Guards |
| Codequalität | ⚠️ Gut | `.unwrap_or_else()` in `storage.rs` maskiert Fehler; inkonsistente Fehlerbehandlung |
| Testabdeckung | ⚠️ Ausreichend | Rust-Unit-Tests solide; TypeScript- & E2E-Abdeckung gering |
| Abhängigkeiten | ✅ Aktuell | Alle Major-Packages auf Stand; keine bekannten Security-Advisories |
| Sicherheit | ✅ Stark | SQLCipher-Verschlüsselung, Path-Traversal-Schutz, kein Telemetrie |
| Dokumentation | ✅ Sehr gut | MkDocs-Site, CLAUDE.md, Inline-Kommentare in Schlüsselmodulen |
| Build/Deploy | ✅ Vollständig | CI/CD für alle Plattformen, idempotente Migrationen, gebundelte Modelle |
| Technische Schulden | ⚠️ Handhabbar | FTS5-Workaround, inline Migrationen, Pfad-Auflösung komplex; kein kritischer Blocker |

---

## 2. Identifizierte Probleme (priorisiert)

### Priorität 1 — Kritisch (Datenverlust / Stabilität)

#### P1-A: Fehler-Maskierung durch `.unwrap_or_else(|| json!({}))` in `storage.rs`

**Ort:** `brief/src-tauri/src/storage.rs`, durchgehend (z. B. Zeile 1005)  
**Problem:** Fehler beim JSON-Deserialisieren werden stillschweigend zu leeren Objekten `{}` degradiert. Datenbeschädigungen oder Schemafehler sind so nicht nachvollziehbar.  
**Risiko:** Stille Datenverluste; schwer debuggbar in Produktion.

```rust
// Vorher (maskiert Fehler)
serde_json::from_str(&output_str).unwrap_or_else(|_| json!({}))

// Nachher (explizite Fehlerbehandlung)
serde_json::from_str(&output_str).map_err(|e| {
    log::error!("JSON parse error in {}: {e}", context_label);
    AppError::DataCorruption(e.to_string())
})?
```

#### P1-B: Mutex-`.unwrap()` in CPAL-Stream-Callback (`audio.rs`)

**Ort:** `brief/src-tauri/src/audio.rs`, Zeilen 91–126 (z. B. Zeile 103)  
**Problem:** `buffer.lock().unwrap()` in einem Thread-Callback-Kontext. Wird das Lock durch einen Panic vergiftet, stürzt die gesamte App ab.  
**Risiko:** Ungeplanter App-Crash während laufender Aufnahme.

```rust
// Vorher
let buf = buffer.lock().unwrap();

// Nachher
let Ok(mut buf) = buffer.lock() else {
    log::error!("Audio buffer mutex poisoned – skipping frame");
    return;
};
```

---

### Priorität 2 — Hoch (Testlücken / Strukturmängel)

#### P2-A: Fehlende E2E-Integrationstests

**Ort:** Kein Test-File deckt den vollständigen Pipeline-Pfad ab  
**Problem:** Der Kernfluss `Aufnahme → Transkription → Zusammenfassung → Persistenz → Abruf` wird nur durch isolierte Unit-Tests geprüft. Regressions werden erst in Produktion sichtbar.  
**Empfehlung:**
1. Rust-Integration-Test in `src-tauri/tests/pipeline_integration.rs`:
   - Mockt WhisperX-Subprocess (via `BRIEF_WHISPERX_RUNNER`-Env)
   - Mockt Ollama-HTTP-Endpunkt (z. B. mit `wiremock`)
   - Prüft kompletten Zyklus gegen In-Memory-SQLite
2. Vitest-Test für `RecordingView → OutputView`-Transition mit gemockten Tauri-Commands

#### P2-B: Zirkuläre Abhängigkeit: `useExport.ts` → `OutputView.tsx`

**Ort:** `brief/src/hooks/useExport.ts`  
**Problem:** Der Hook importiert `safeExportBaseName` aus dem Component-Layer (`OutputView.tsx`). Hooks sollten keine Komponenten als Abhängigkeiten haben.  
**Fix:** Utility nach `brief/src/utils/exportUtils.ts` verschieben; sowohl `useExport.ts` als auch `OutputView.tsx` importieren daraus.

```
Vorher:
useExport.ts → OutputView.tsx (safeExportBaseName)

Nachher:
useExport.ts  ┐
              ├─→ src/utils/exportUtils.ts (safeExportBaseName)
OutputView.tsx ┘
```

#### P2-C: Inkonsistente Fehlerbehandlung in Commands (`recording.rs`)

**Ort:** `brief/src-tauri/src/commands/recording.rs`  
**Problem:** Mischung aus `?`-Operator und `.map_err(|_| AppError::...)` ohne klares Schema. Kontext geht verloren.  
**Fix:** Einheitlich `map_err` mit beschreibenden Fehlernachrichten; `?` nur wenn der Fehlertyp direkt konvertierbar ist.

---

### Priorität 3 — Mittel (Technische Schulden)

#### P3-A: WhisperX-Skript-Pfad-Auflösung zu komplex

**Ort:** `brief/src-tauri/src/transcribe.rs`, Zeilen 48–64  
**Problem:** 4 Fallback-Strategien (Env-Var → Relativer Pfad → Bundled Resources → ...) erschweren das Debugging, wenn der falsche Pfad gewählt wird.  
**Empfehlung:**
- Produktion: Ausschließlich `bundled_resource_path()` verwenden
- Entwicklung: `BRIEF_WHISPERX_RUNNER`-Env-Var als einziger Override
- Auflösungsstrategie einmalig loggen (auf `info`-Level)

#### P3-B: Inline-SQLite-Migrationen statt `sqlx migrate`

**Ort:** `brief/src-tauri/src/storage.rs`, `run_migrations()`-Methode  
**Problem:** Alle Migrationen sind inline im Rust-Code. Kein Versions-Management, keine Rollback-Möglichkeit.  
**Empfehlung:** Migration zu `sqlx migrate add`-Workflow:
- Migrationsdateien unter `src-tauri/migrations/*.sql`
- Eingebettet via `sqlx::migrate!()` Makro
- Kompatibel mit SQLCipher (Connection-Hook weiterhin setzen)

#### P3-C: Keine Retry-Logik für WhisperX-Subprocess

**Ort:** `brief/src-tauri/src/commands/recording.rs`, Zeile 179  
**Problem:** WhisperX wird einmalig gestartet; bei Timeout oder transientem Fehler keine Wiederholung. Ollama hat bereits Retry-Logik.  
**Fix:** Identisches Retry-with-Backoff-Muster wie in `summarize.rs` anwenden (max. 2 Retries, kein Retry bei `ParseError`).

#### P3-D: FTS5-Workaround dokumentieren / bereinigen

**Ort:** `brief/src-tauri/src/storage.rs`, Kommentar bei Zeile 68  
**Problem:** „External-content sync did not populate the inverted index reliably with SQLCipher" — Workaround existiert, ist aber nicht als ADR dokumentiert.  
**Fix:** ADR (Architecture Decision Record) in `brief/docs/adrs.md` hinzufügen.

---

### Priorität 4 — Niedrig (Qualitäts-Hygiene)

#### P4-A: Windows-ACL für Verschlüsselungsschlüssel-Fallback-Datei

**Ort:** `brief/src-tauri/src/crypto_key.rs`  
**Problem:** Fallback-Datei erhält auf Linux/macOS `0o600`; auf Windows gibt es kein Äquivalent im aktuellen Code. Standard-Windows-ACLs erlauben Lesezugriff für Gruppenkonten.  
**Fix:** Plattformspezifischen Block mit `#[cfg(windows)]` hinzufügen, der explizit `DACL` über die `windows-acl`-Crate oder `std::os::windows::fs::OpenOptionsExt` setzt.

#### P4-B: ReDoS-Schutz in `OutputView.tsx`

**Ort:** `brief/src/views/OutputView.tsx`, Zeile 38–46  
**Problem:** `highlightTerms()` escaped zwar Sonderzeichen, begrenzt aber nicht die Länge des Suchbegriffs. Pathologische Regex-Muster könnten den UI-Thread blockieren.  
**Fix:** Maximale Länge des Suchterms auf z. B. 100 Zeichen begrenzen:

```typescript
const safeTerm = query.slice(0, 100).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
```

#### P4-C: ESLint und Prettier konfigurieren

**Ort:** Projekt-Root (fehlt)  
**Problem:** Kein Linter und kein Formatter für TypeScript konfiguriert. TypeScript Strict-Mode ist aktiv, prüft aber keine stilistischen Probleme.  
**Fix:**
```bash
npm install -D eslint @typescript-eslint/parser @typescript-eslint/eslint-plugin prettier eslint-config-prettier
```
CI-Step in `ci.yml` ergänzen: `npm run lint`.

#### P4-D: Keine `rustfmt`-Konfiguration

**Ort:** `brief/src-tauri/` (fehlt `rustfmt.toml`)  
**Fix:** `rustfmt.toml` mit Standardwerten anlegen; `cargo fmt --check` in CI ergänzen.

---

## 3. Maßnahmenplan

### Phase 1 — Sofortmaßnahmen (Sprint 1, ~3–5 Tage)

| ID | Maßnahme | Datei(en) | Aufwand |
|---|---|---|---|
| P1-A | Fehler-Maskierung in `storage.rs` beheben | `storage.rs` | M |
| P1-B | Mutex-Panic in `audio.rs` absichern | `audio.rs` | S |
| P2-B | Zirkuläre Abhängigkeit auflösen | `useExport.ts`, `OutputView.tsx`, neues `exportUtils.ts` | S |
| P4-B | ReDoS-Schutz für Suche | `OutputView.tsx` | XS |

### Phase 2 — Kurzfristig (Sprint 2–3, ~1–2 Wochen)

| ID | Maßnahme | Datei(en) | Aufwand |
|---|---|---|---|
| P2-A | E2E-Integrationstests anlegen | `tests/pipeline_integration.rs`, neue Vitest-Tests | L |
| P2-C | Einheitliche Fehlerbehandlung in Commands | `commands/recording.rs`, ggf. weitere Commands | M |
| P3-A | WhisperX-Pfadauflösung vereinfachen | `transcribe.rs` | S |
| P3-C | Retry-Logik für WhisperX | `commands/recording.rs` | S |
| P4-C | ESLint + Prettier einrichten | Root, `ci.yml` | S |
| P4-D | `rustfmt` konfigurieren | `src-tauri/rustfmt.toml`, `ci.yml` | XS |

### Phase 3 — Mittelfristig (Sprint 4–6, ~3–4 Wochen)

| ID | Maßnahme | Datei(en) | Aufwand |
|---|---|---|---|
| P3-B | Migration zu `sqlx migrate`-Workflow | `storage.rs`, neues `migrations/`-Verzeichnis | L |
| P3-D | ADR für FTS5-Workaround | `docs/adrs.md` | XS |
| P4-A | Windows-ACL-Härtung | `crypto_key.rs` | M |

---

## 4. Aufwandslegende

| Kürzel | Beschreibung |
|---|---|
| XS | < 1 Stunde |
| S | 1–4 Stunden |
| M | 0,5–2 Tage |
| L | 3–5 Tage |

---

## 5. Nicht-Maßnahmen (bewusste Entscheidungen)

- **WhisperX-Modell-Caching via CDN:** Komplexitätserhöhung überwiegt Nutzen für Offline-First-App; vorerst nicht angehen.
- **Async Python Runner:** Rust→Python-Subprocess-Architektur bleibt; Umbau auf async Python-Client würde erheblichen Refactoring-Aufwand erfordern ohne klaren Stabilitätsgewinn.
- **Redux/MobX State Management:** Lokaler Hook-State reicht für aktuelle App-Komplexität; kein Bedarf für externes State-Management-Framework.

---

*Generiert durch automatisierte Codeinspektion — alle Zeilenangaben auf Branch `claude/audit-architecture-plan-PtL2r` gültig.*
