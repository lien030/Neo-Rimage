import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import enTranslation from "./en.json";
import jaTranslation from "./ja.json";
import zhTranslation from "./zh.json";

export const SUPPORTED_LANGUAGES = ["zh", "en", "ja"] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];
export const LANGUAGE_STORAGE_KEY = "language";

const DEFAULT_LANGUAGE: SupportedLanguage = "en";

const i18nResources = {
  zh: {
    translation: zhTranslation,
  },
  en: {
    translation: enTranslation,
  },
  ja: {
    translation: jaTranslation,
  },
};

function isSupportedLanguage(
  language: string | null,
): language is SupportedLanguage {
  return SUPPORTED_LANGUAGES.some((supported) => supported === language);
}

function getInitialLanguage(): SupportedLanguage {
  const storedLanguage =
    typeof localStorage === "undefined"
      ? null
      : localStorage.getItem(LANGUAGE_STORAGE_KEY);

  return isSupportedLanguage(storedLanguage)
    ? storedLanguage
    : DEFAULT_LANGUAGE;
}

void i18n.use(initReactI18next).init({
  resources: i18nResources,
  lng: getInitialLanguage(),
  fallbackLng: DEFAULT_LANGUAGE,
  supportedLngs: [...SUPPORTED_LANGUAGES],
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
