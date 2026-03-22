import { useTranslation } from "react-i18next";

export function HistoryView() {
  const { t } = useTranslation();
  return (
    <section aria-label={t("nav.history")}>{t("views.history")}</section>
  );
}
