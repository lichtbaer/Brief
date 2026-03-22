import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppSettingsSnapshot } from "../types";

type OnboardingStep =
  | "welcome"
  | "whisperx"
  | "ollama"
  | "meeting_type"
  | "done";

type OllamaCheckResult = {
  running: boolean;
  recommended_model: string;
};

type Props = {
  onComplete: () => void;
};

export function OnboardingWizard({ onComplete }: Props) {
  const { t } = useTranslation();
  const [step, setStep] = useState<OnboardingStep>("welcome");
  const [whisperxOk, setWhisperxOk] = useState<boolean | null>(null);
  const [ollamaStatus, setOllamaStatus] = useState<OllamaCheckResult | null>(null);
  const [checkingWhisperx, setCheckingWhisperx] = useState(false);
  const [checkingOllama, setCheckingOllama] = useState(false);

  const checkWhisperX = async () => {
    setCheckingWhisperx(true);
    setWhisperxOk(null);
    try {
      const ok = await invoke<boolean>("check_whisperx");
      setWhisperxOk(ok);
    } catch {
      setWhisperxOk(false);
    } finally {
      setCheckingWhisperx(false);
    }
  };

  const checkOllama = async () => {
    setCheckingOllama(true);
    setOllamaStatus(null);
    try {
      const status = await invoke<OllamaCheckResult>("check_ollama");
      setOllamaStatus(status);
    } catch {
      try {
        const snap = await invoke<AppSettingsSnapshot>("get_app_settings_snapshot");
        setOllamaStatus({ running: false, recommended_model: snap.recommendedModel });
      } catch {
        setOllamaStatus({ running: false, recommended_model: "llama3.1:8b" });
      }
    } finally {
      setCheckingOllama(false);
    }
  };

  const complete = async () => {
    await invoke("update_setting", { key: "onboarding_complete", value: "true" });
    onComplete();
  };

  return (
    <div className="onboarding-wizard">
      {step === "welcome" && (
        <div className="onboarding-step">
          <h1>{t("onboarding.welcome_title")}</h1>
          <p>{t("onboarding.welcome_text")}</p>
          <button
            type="button"
            className="onboarding-primary"
            onClick={() => {
              setStep("whisperx");
              void checkWhisperX();
            }}
          >
            {t("onboarding.next")}
          </button>
        </div>
      )}

      {step === "whisperx" && (
        <div className="onboarding-step">
          <h2>{t("onboarding.whisperx_title")}</h2>

          {checkingWhisperx && (
            <p style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
              <span className="spinner spinner-dark" />
              {t("onboarding.checking")}
            </p>
          )}

          {!checkingWhisperx && whisperxOk === true && (
            <>
              <p className="status-ok">{t("onboarding.whisperx_ok")}</p>
              <button
                type="button"
                className="onboarding-primary"
                onClick={() => {
                  setStep("ollama");
                  void checkOllama();
                }}
              >
                {t("onboarding.next")}
              </button>
            </>
          )}

          {!checkingWhisperx && whisperxOk === false && (
            <>
              <p className="status-error">{t("onboarding.whisperx_missing")}</p>
              <pre className="setup-command">cd whisperx_runner && bash setup.sh</pre>
              <div className="onboarding-actions-row">
                <button
                  type="button"
                  onClick={() => void checkWhisperX()}
                  disabled={checkingWhisperx}
                >
                  {t("onboarding.retry")}
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {step === "ollama" && (
        <div className="onboarding-step">
          <h2>{t("onboarding.ollama_title")}</h2>

          {checkingOllama && (
            <p style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
              <span className="spinner spinner-dark" />
              {t("onboarding.checking")}
            </p>
          )}

          {!checkingOllama && ollamaStatus?.running === true && (
            <>
              <p className="status-ok">{t("onboarding.ollama_ok")}</p>
              <p>
                {t("onboarding.recommended_model")}:{" "}
                <code>{ollamaStatus.recommended_model}</code>
              </p>
              <button
                type="button"
                className="onboarding-primary"
                onClick={() => setStep("meeting_type")}
              >
                {t("onboarding.next")}
              </button>
            </>
          )}

          {!checkingOllama && ollamaStatus?.running === false && (
            <>
              <p className="status-error">{t("onboarding.ollama_missing")}</p>
              <pre className="setup-command">ollama serve</pre>
              <p>
                {t("onboarding.ollama_model_hint")}:{" "}
                <code>ollama pull {ollamaStatus.recommended_model}</code>
              </p>
              <div className="onboarding-actions-row">
                <button
                  type="button"
                  onClick={() => void checkOllama()}
                  disabled={checkingOllama}
                >
                  {t("onboarding.retry")}
                </button>
                <button
                  type="button"
                  className="onboarding-skip"
                  onClick={() => setStep("meeting_type")}
                >
                  {t("onboarding.skip_ollama")}
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {step === "meeting_type" && (
        <div className="onboarding-step">
          <h2>{t("onboarding.meeting_type_title")}</h2>
          <p>{t("onboarding.meeting_type_text")}</p>
          <div className="onboarding-meeting-types">
            {(["consulting", "legal", "internal"] as const).map((type) => (
              <button
                key={type}
                type="button"
                className="meeting-type-choice"
                onClick={async () => {
                  await invoke("update_setting", {
                    key: "default_meeting_type",
                    value: type,
                  });
                  setStep("done");
                }}
              >
                {t(`meeting_types.${type}`)}
              </button>
            ))}
          </div>
        </div>
      )}

      {step === "done" && (
        <div className="onboarding-step">
          <h2>{t("onboarding.done_title")}</h2>
          <p>{t("onboarding.done_text")}</p>
          <button
            type="button"
            className="onboarding-primary"
            onClick={() => void complete()}
          >
            {t("onboarding.start")}
          </button>
        </div>
      )}
    </div>
  );
}
