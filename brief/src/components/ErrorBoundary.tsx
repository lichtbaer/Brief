import React from "react";
import { useTranslation } from "react-i18next";

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

interface ErrorFallbackProps {
  error: Error | null;
  onReset: () => void;
}

/**
 * Renders a recovery UI when a child component throws an unhandled error.
 * Uses functional component + hooks so translation works normally.
 */
function ErrorFallback({ error, onReset }: ErrorFallbackProps) {
  const { t } = useTranslation();
  return (
    <div
      role="alert"
      style={{
        padding: "2rem",
        display: "flex",
        flexDirection: "column",
        gap: "1rem",
        maxWidth: "40rem",
        margin: "4rem auto",
      }}
    >
      <h2 style={{ margin: 0 }}>{t("errors.boundary_title")}</h2>
      <p style={{ margin: 0, opacity: 0.8 }}>{t("errors.boundary_message")}</p>
      {error && (
        <pre
          style={{
            fontSize: "0.75rem",
            background: "var(--color-surface, #f1f5f9)",
            padding: "0.75rem",
            borderRadius: "0.375rem",
            overflow: "auto",
            whiteSpace: "pre-wrap",
          }}
        >
          {error.message}
        </pre>
      )}
      <button type="button" className="btn btn-primary" onClick={onReset}>
        {t("errors.boundary_retry")}
      </button>
    </div>
  );
}

/**
 * Class-based error boundary wrapping application views.
 * Catches synchronous render errors and unhandled rejections from child components,
 * preventing a single broken view from crashing the entire Tauri window.
 */
export class ErrorBoundary extends React.Component<
  React.PropsWithChildren<object>,
  ErrorBoundaryState
> {
  constructor(props: React.PropsWithChildren<object>) {
    super(props);
    this.state = { hasError: false, error: null };
    this.handleReset = this.handleReset.bind(this);
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo): void {
    // Log to console so it surfaces in Tauri's WebView DevTools and any attached debugger.
    console.error("[ErrorBoundary] Unhandled render error:", error, info.componentStack);
  }

  handleReset(): void {
    this.setState({ hasError: false, error: null });
  }

  render() {
    if (this.state.hasError) {
      return (
        <ErrorFallback error={this.state.error} onReset={this.handleReset} />
      );
    }
    return this.props.children;
  }
}
