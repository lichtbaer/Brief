import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import de from "./locales/de/common.json";
import en from "./locales/en/common.json";

const LOCALE_STORAGE_KEY = "brief.locale";

function getStoredLanguage(): "de" | "en" {
  if (typeof window === "undefined") {
    return "de";
  }
  const stored = localStorage.getItem(LOCALE_STORAGE_KEY);
  if (stored === "en" || stored === "de") {
    return stored;
  }
  return "de";
}

void i18n.use(initReactI18next).init({
  resources: {
    de: { common: de },
    en: { common: en },
  },
  lng: getStoredLanguage(),
  fallbackLng: "en",
  ns: ["common"],
  defaultNS: "common",
  interpolation: { escapeValue: false },
});

i18n.on("languageChanged", (lng) => {
  localStorage.setItem(LOCALE_STORAGE_KEY, lng);
});

export default i18n;
