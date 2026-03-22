import { useTranslation } from "react-i18next";

export function OutputView() {
  const { t } = useTranslation();
  return (
    <section aria-label={t("nav.output")}>{t("views.output")}</section>
  );
}
