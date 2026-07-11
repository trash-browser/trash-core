import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { IconPlus, IconSearch } from "@tabler/icons-react";
import { NTP_SHORTCUTS } from "../data/managers";

export function NewTabPage() {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    const q = query.trim();
    if (!q) return;
    const url =
      q.includes(".") && !q.includes(" ")
        ? q.startsWith("http")
          ? q
          : `https://${q}`
        : `https://www.bing.com/search?q=${encodeURIComponent(q)}`;
    window.open(url, "_blank", "noopener,noreferrer");
  };

  return (
    <div className="flex h-full min-h-0 flex-col overflow-y-auto bg-shell">
      <div className="mx-auto flex w-full max-w-[720px] flex-1 flex-col items-center px-6 pt-[14vh] pb-16">
        <div className="mb-10 text-center">
          <h1 className="text-4xl font-semibold tracking-tight text-shell-text">
            Trash <span className="text-shell-accent">Browser</span>
          </h1>
          <p className="mt-2 text-sm text-shell-muted">{t("ntp.tagline")}</p>
        </div>

        <form onSubmit={onSubmit} className="w-full">
          <label className="flex h-12 items-center gap-3 rounded-full border border-shell-border bg-shell-elevated px-5 shadow-[0_8px_24px_rgba(0,0,0,0.25)] focus-within:border-shell-accent">
            <IconSearch size={18} stroke={1.6} className="text-shell-muted" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("ntp.searchPlaceholder")}
              className="h-full w-full bg-transparent text-[15px] text-shell-text outline-none placeholder:text-shell-muted"
            />
          </label>
        </form>

        <div className="mt-12 grid w-full grid-cols-4 gap-x-6 gap-y-8 sm:grid-cols-4">
          {NTP_SHORTCUTS.map((item) => (
            <a
              key={item.id}
              href={item.url}
              target="_blank"
              rel="noreferrer"
              className="group flex flex-col items-center gap-2"
            >
              <span
                className="flex size-16 items-center justify-center rounded-2xl border border-shell-border text-xl font-semibold text-shell-text transition-colors group-hover:border-shell-accent group-hover:bg-shell-hover"
                style={{ background: item.tint }}
              >
                {item.letter}
              </span>
              <span className="max-w-[88px] truncate text-center text-xs text-shell-muted group-hover:text-shell-text">
                {t(item.labelKey)}
              </span>
            </a>
          ))}
          <button
            type="button"
            className="group flex flex-col items-center gap-2"
            aria-label={t("ntp.addShortcut")}
          >
            <span className="flex size-16 items-center justify-center rounded-2xl border border-dashed border-shell-border text-shell-muted transition-colors group-hover:border-shell-accent group-hover:text-shell-accent">
              <IconPlus size={22} stroke={1.6} />
            </span>
            <span className="text-xs text-shell-muted">{t("ntp.addShortcut")}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
