import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";
import th from "./locales/th.json";
import zh from "./locales/zh.json";
import ja from "./locales/ja.json";
import { setSetting } from "../native/ipc";

export const LOCALES = [
  { code: "en", labelKey: "langs.en" },
  { code: "th", labelKey: "langs.th" },
  { code: "zh", labelKey: "langs.zh" },
  { code: "ja", labelKey: "langs.ja" },
] as const;

export type LocaleCode = (typeof LOCALES)[number]["code"];

const STORAGE_KEY = "trash-shell.locale";

function detectLocale(): LocaleCode {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved && LOCALES.some((l) => l.code === saved)) {
    return saved as LocaleCode;
  }
  const nav = navigator.language.toLowerCase();
  if (nav.startsWith("th")) return "th";
  if (nav.startsWith("zh")) return "zh";
  if (nav.startsWith("ja")) return "ja";
  return "en";
}

void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    th: { translation: th },
    zh: { translation: zh },
    ja: { translation: ja },
  },
  lng: detectLocale(),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export function setLocale(code: LocaleCode) {
  localStorage.setItem(STORAGE_KEY, code);
  void setSetting("settings.languages.preferred", code);
  document.documentElement.lang = code === "zh" ? "zh-CN" : code;
  return i18n.changeLanguage(code);
}

document.documentElement.lang =
  i18n.language === "zh" ? "zh-CN" : i18n.language;

export default i18n;
