import { useEffect, useState } from "react";
import { IconClock, IconSearch, IconTrash, IconX } from "@tabler/icons-react";
import { useTranslation } from "react-i18next";
import {
  clearHistory,
  getHistory,
  navigate,
  removeHistoryEntry,
  type HistoryEntry,
} from "../native/ipc";

export function HistoryPage() {
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState("");
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void getHistory(query).then((next) => {
        if (!cancelled) {
          setEntries(next);
          setLoading(false);
        }
      });
    }, 120);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [query]);

  const remove = async (id: number) => {
    if (await removeHistoryEntry(id)) {
      setEntries((current) => current.filter((entry) => entry.id !== id));
    }
  };

  const clear = async () => {
    if (await clearHistory()) setEntries([]);
  };

  return (
    <div className="h-full overflow-y-auto bg-shell text-shell-text">
      <div className="mx-auto w-full max-w-[820px] px-7 py-8">
        <header className="mb-6 flex items-end justify-between gap-4">
          <div>
            <h1 className="page-title">{t("history.title", { defaultValue: "History" })}</h1>
            <p className="page-sub">
              {t("history.subtitle", { defaultValue: "Pages visited in this browser" })}
            </p>
          </div>
          <button type="button" onClick={() => void clear()} className="rounded-xl px-3 py-2 text-xs text-shell-muted hover:bg-shell-hover hover:text-shell-danger">
            <IconTrash className="mr-1.5 inline" size={14} />
            {t("history.clear", { defaultValue: "Clear history" })}
          </button>
        </header>

        <label className="mb-4 flex h-10 items-center gap-2.5 rounded-xl border border-shell-border/80 bg-shell-elevated px-3.5 focus-within:border-shell-accent/60">
          <IconSearch size={15} className="text-shell-muted" />
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("history.search", { defaultValue: "Search history" })} className="h-full w-full bg-transparent text-[13px] outline-none placeholder:text-shell-muted" />
        </label>

        <section className="panel">
          {loading ? (
            <div className="px-5 py-14 text-center text-sm text-shell-muted">Loading…</div>
          ) : entries.length === 0 ? (
            <div className="px-5 py-14 text-center text-sm text-shell-muted">
              {t("history.empty", { defaultValue: "No history" })}
            </div>
          ) : (
            <ul className="divide-y divide-shell-border/50">
              {entries.map((entry) => (
                <li key={entry.id} className="group flex items-center gap-3 px-4 py-3 hover:bg-shell-hover/50">
                  <IconClock size={16} className="shrink-0 text-shell-muted" />
                  <button type="button" onClick={() => void navigate(entry.url)} className="min-w-0 flex-1 text-left">
                    <div className="truncate text-[13px] text-shell-text">{entry.title || entry.url}</div>
                    <div className="mt-0.5 truncate text-xs text-shell-muted">{entry.url}</div>
                  </button>
                  <time className="shrink-0 text-[11px] text-shell-muted">
                    {new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(entry.visited_at * 1000)}
                  </time>
                  <button type="button" onClick={() => void remove(entry.id)} aria-label="Remove" className="rounded-lg p-1.5 text-shell-muted opacity-0 hover:bg-shell-hover hover:text-shell-danger group-hover:opacity-100">
                    <IconX size={14} />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
