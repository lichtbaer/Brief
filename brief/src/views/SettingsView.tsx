import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppSettingsSnapshot } from "../types";

export function SettingsView() {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<AppSettingsSnapshot | null>(null);
  const [modelDraft, setModelDraft] = useState("");
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "ok" | "err">(
    "idle",
  );

  const load = () => {
    void invoke<AppSettingsSnapshot>("get_app_settings_snapshot")
      .then((s) => {
        setSnapshot(s);
        setModelDraft(s.llmModel);
      })
      .catch(() => setSnapshot(null));
  };

  useEffect(() => {
    load();
  }, []);

  const handleSave = async () => {
    setSaveStatus("saving");
    try {
      await invoke("set_llm_model", { model: modelDraft });
      setSaveStatus("ok");
      load();
    } catch {
      setSaveStatus("err");
    }
  };

  return (
    <section aria-label={t("nav.settings")}>
      <h2 style={{ marginTop: 0 }}>{t("views.settings")}</h2>
      {snapshot ? (
        <>
          <p>{t("settings.memory_detected", { gb: snapshot.memoryGb.toFixed(1) })}</p>
          <p>
            <label htmlFor="brief-llm-model">{t("settings.llm_model_label")}</label>
          </p>
          <p>
            <input
              id="brief-llm-model"
              type="text"
              value={modelDraft}
              onChange={(e) => setModelDraft(e.target.value)}
              style={{ width: "min(100%, 24rem)" }}
              autoComplete="off"
              spellCheck={false}
            />
          </p>
          <p>
            <button
              type="button"
              onClick={() => void handleSave()}
              disabled={saveStatus === "saving"}
            >
              {t("settings.llm_model_save")}
            </button>
          </p>
          {saveStatus === "ok" && <p role="status">{t("settings.llm_model_saved")}</p>}
          {saveStatus === "err" && (
            <p role="alert">{t("settings.llm_model_save_error")}</p>
          )}
          {snapshot.llmModelUserOverride && (
            <p>{t("settings.llm_model_override_hint")}</p>
          )}
        </>
      ) : (
        <p>{t("settings.loading")}</p>
      )}
    </section>
  );
}
