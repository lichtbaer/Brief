/**
 * Converts a meeting title into a filesystem-safe base name for export files.
 * Strips characters that are illegal on Windows, macOS, or Linux, then trims whitespace.
 * Falls back to "meeting" if the sanitised result would be empty.
 */
export function safeExportBaseName(title: string): string {
  const trimmed = title.replace(/[/\\?%*:|"<>]/g, "-").trim();
  return trimmed.length > 0 ? trimmed : "meeting";
}
