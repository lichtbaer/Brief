import { useTranslation } from "react-i18next";

export function SettingsView() {
  const { t } = useTranslation();
  return (
    <section aria-label={t("nav.settings")}>{t("views.settings")}</section>
  );
}
