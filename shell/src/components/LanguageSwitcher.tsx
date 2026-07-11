import { useTranslation } from "react-i18next";
import { LOCALES, setLocale, type LocaleCode } from "../i18n";

type Props = {
  /** Compact sidebar style vs full settings card. */
  variant?: "card" | "inline";
};

export function LanguageSwitcher({ variant = "card" }: Props) {
  const { t, i18n } = useTranslation();
  const current = (i18n.resolvedLanguage || i18n.language || "en").slice(
    0,
    2,
  ) as LocaleCode;

  if (variant === "inline") {
    return (
      <div className="grid grid-cols-2 gap-1">
        {LOCALES.map((locale) => {
          const active = current === locale.code;
          return (
            <button
              key={locale.code}
              type="button"
              onClick={() => void setLocale(locale.code)}
              className={[
                "rounded-lg px-2 py-1.5 text-left text-[12px] transition-colors",
                active
                  ? "bg-shell-accent-dim text-shell-accent"
                  : "text-shell-text hover:bg-shell-hover",
              ].join(" ")}
            >
              {t(locale.labelKey)}
            </button>
          );
        })}
      </div>
    );
  }

  return (
    <section className="panel">
      <div className="border-b border-shell-border/50 px-5 py-3">
        <div className="text-[13px] font-medium tracking-[-0.01em] text-shell-text">
          {t("shell.language")}
        </div>
        <div className="mt-0.5 text-xs text-shell-muted">
          {t("pages.languages.preferred.pickerHint")}
        </div>
      </div>
      <div className="divide-y divide-shell-border/50">
        {LOCALES.map((locale) => {
          const active = current === locale.code;
          return (
            <button
              key={locale.code}
              type="button"
              onClick={() => void setLocale(locale.code)}
              className="flex w-full items-center gap-4 px-5 py-3.5 text-left hover:bg-shell-hover/60"
            >
              <span
                className={[
                  "flex size-4 shrink-0 items-center justify-center rounded-full border",
                  active ? "border-shell-accent" : "border-shell-track",
                ].join(" ")}
              >
                {active ? (
                  <span className="size-2 rounded-full bg-shell-accent" />
                ) : null}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block text-[13px] tracking-[-0.01em] text-shell-text">
                  {t(locale.labelKey)}
                </span>
                <span className="block text-xs text-shell-muted">
                  {locale.code.toUpperCase()}
                </span>
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
