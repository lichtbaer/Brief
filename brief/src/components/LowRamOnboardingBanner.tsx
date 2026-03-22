import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

type Props = {
  recommendedModel: string;
  onDismissed: () => void;
};

export function LowRamOnboardingBanner({
  recommendedModel,
  onDismissed,
}: Props) {
  const { t } = useTranslation();
  const pullCommand = `ollama pull ${recommendedModel}`;

  const handleDismiss = async () => {
    await invoke("dismiss_low_ram_onboarding");
    onDismissed();
  };

  return (
    <aside
      role="status"
      aria-live="polite"
      style={{
        marginBottom: "1rem",
        padding: "0.75rem 1rem",
        border: "1px solid #c9a227",
        borderRadius: "6px",
        background: "#fffbeb",
      }}
    >
      <h2 style={{ margin: "0 0 0.5rem", fontSize: "1rem" }}>
        {t("onboarding.low_ram.title")}
      </h2>
      <p style={{ margin: "0 0 0.5rem" }}>{t("onboarding.low_ram.body")}</p>
      <p style={{ margin: "0 0 0.5rem" }}>
        <strong>{t("onboarding.low_ram.recommended_label")}</strong>{" "}
        {recommendedModel}
      </p>
      <pre
        style={{
          margin: "0 0 0.75rem",
          padding: "0.5rem",
          background: "#f4f4f5",
          borderRadius: "4px",
          overflow: "auto",
        }}
      >
        <code>{pullCommand}</code>
      </pre>
      <button type="button" onClick={() => void handleDismiss()}>
        {t("onboarding.low_ram.dismiss")}
      </button>
    </aside>
  );
}
