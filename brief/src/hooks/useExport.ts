/**
 * useExport — encapsulates Markdown, PDF, and audio export logic previously inlined in OutputView.
 * Returns busy/error state and three export trigger functions.
 */
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { useCallback, useState } from "react";

import { safeExportBaseName } from "../utils/exportUtils";

export type ExportFormat = "markdown" | "pdf" | "audio" | "csv";

interface UseExportResult {
  exportBusy: ExportFormat | null;
  exportError: string | null;
  /** Set when an audio export completes successfully — contains the saved file path. */
  exportSuccess: string | null;
  exportMarkdown: (meetingId: string, title: string) => Promise<void>;
  exportPdf: (meetingId: string, title: string) => Promise<void>;
  exportAudio: (meetingId: string) => Promise<string | null>;
  exportCsv: (meetingId: string) => Promise<void>;
}

export function useExport(): UseExportResult {
  const [exportBusy, setExportBusy] = useState<ExportFormat | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportSuccess, setExportSuccess] = useState<string | null>(null);

  const showError = useCallback((e: unknown) => {
    setExportError(String(e));
    setTimeout(() => setExportError(null), 5000);
  }, []);

  const showSuccess = useCallback((path: string) => {
    setExportSuccess(path);
    setTimeout(() => setExportSuccess(null), 5000);
  }, []);

  const exportMarkdown = useCallback(async (meetingId: string, title: string) => {
    setExportBusy("markdown");
    try {
      const markdown = await invoke<string>("export_markdown", { id: meetingId });
      const base = safeExportBaseName(title);
      const path = await save({
        defaultPath: `${base}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (path) {
        await writeFile(path, new TextEncoder().encode(markdown));
      }
    } catch (e) {
      showError(e);
    } finally {
      setExportBusy(null);
    }
  }, [showError]);

  const exportPdf = useCallback(async (meetingId: string, title: string) => {
    setExportBusy("pdf");
    try {
      const pdfBase64 = await invoke<string>("export_pdf", { id: meetingId });
      const base = safeExportBaseName(title);
      const path = await save({
        defaultPath: `${base}.pdf`,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (path) {
        let decoded: string;
        try {
          decoded = atob(pdfBase64);
        } catch {
          throw new Error("PDF base64 decoding failed");
        }
        const bytes = Uint8Array.from(decoded, (c) => c.charCodeAt(0));
        await writeFile(path, bytes);
      }
    } catch (e) {
      showError(e);
    } finally {
      setExportBusy(null);
    }
  }, [showError]);

  const exportAudio = useCallback(async (meetingId: string): Promise<string | null> => {
    setExportBusy("audio");
    try {
      const savedPath = await invoke<string>("export_audio", { id: meetingId });
      // Use in-app success state instead of window.alert() to avoid blocking the UI thread.
      showSuccess(savedPath);
      return savedPath;
    } catch (e) {
      if (!String(e).includes("cancelled")) {
        showError(e);
      }
      return null;
    } finally {
      setExportBusy(null);
    }
  }, [showError, showSuccess]);

  const exportCsv = useCallback(async (meetingId: string) => {
    setExportBusy("csv");
    try {
      const savedPath = await invoke<string>("export_action_items_csv", { id: meetingId });
      showSuccess(savedPath);
    } catch (e) {
      if (!String(e).includes("cancelled")) {
        showError(e);
      }
    } finally {
      setExportBusy(null);
    }
  }, [showError, showSuccess]);

  return { exportBusy, exportError, exportSuccess, exportMarkdown, exportPdf, exportAudio, exportCsv };
}
