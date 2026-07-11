import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  IconFolder,
  IconPlayerPlay,
  IconSearch,
  IconTrash,
  IconX,
} from "@tabler/icons-react";
import { MOCK_DOWNLOADS, type DownloadItem } from "../data/managers";

type Filter = "all" | "progress" | "complete" | "paused" | "failed";

function StatusBadge({ item }: { item: DownloadItem }) {
  const { t } = useTranslation();
  const map = {
    complete: { label: t("downloadsMgr.status.complete"), cls: "text-shell-ok" },
    progress: { label: t("downloadsMgr.status.progress"), cls: "text-shell-accent" },
    paused: { label: t("downloadsMgr.status.paused"), cls: "text-shell-muted" },
    failed: { label: t("downloadsMgr.status.failed"), cls: "text-shell-danger" },
  } as const;
  const s = map[item.status];
  return <span className={`text-xs ${s.cls}`}>{s.label}</span>;
}

export function DownloadManagerPage() {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [items, setItems] = useState(MOCK_DOWNLOADS);

  const filtered = useMemo(() => {
    return items.filter((item) => {
      if (filter !== "all" && item.status !== filter) return false;
      const q = query.trim().toLowerCase();
      if (!q) return true;
      return (
        item.name.toLowerCase().includes(q) ||
        item.source.toLowerCase().includes(q)
      );
    });
  }, [items, filter, query]);

  const remove = (id: string) =>
    setItems((prev) => prev.filter((item) => item.id !== id));

  const clearCompleted = () =>
    setItems((prev) => prev.filter((item) => item.status !== "complete"));

  const filters: { id: Filter; labelKey: string }[] = [
    { id: "all", labelKey: "downloadsMgr.filters.all" },
    { id: "progress", labelKey: "downloadsMgr.filters.progress" },
    { id: "complete", labelKey: "downloadsMgr.filters.complete" },
    { id: "paused", labelKey: "downloadsMgr.filters.paused" },
    { id: "failed", labelKey: "downloadsMgr.filters.failed" },
  ];

  return (
    <div className="flex h-full min-h-0 flex-col overflow-y-auto bg-shell">
      <div className="mx-auto w-full max-w-[860px] px-6 py-8">
        <header className="mb-6 flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="text-[28px] font-normal tracking-tight text-shell-text">
              {t("downloadsMgr.title")}
            </h1>
            <p className="mt-1 text-sm text-shell-muted">
              {t("downloadsMgr.subtitle")}
            </p>
          </div>
          <button
            type="button"
            onClick={clearCompleted}
            className="rounded-full px-3 py-1.5 text-[13px] text-shell-accent hover:bg-shell-accent-dim"
          >
            {t("downloadsMgr.clearCompleted")}
          </button>
        </header>

        <label className="mb-4 flex h-10 items-center gap-3 rounded-full border border-shell-border bg-shell-elevated px-4 focus-within:border-shell-accent">
          <IconSearch size={16} stroke={1.6} className="text-shell-muted" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("downloadsMgr.search")}
            className="h-full w-full bg-transparent text-[13px] text-shell-text outline-none placeholder:text-shell-muted"
          />
        </label>

        <div className="mb-4 flex flex-wrap gap-1">
          {filters.map((f) => (
            <button
              key={f.id}
              type="button"
              onClick={() => setFilter(f.id)}
              className={[
                "rounded-full px-3 py-1.5 text-[12px] transition-colors",
                filter === f.id
                  ? "bg-shell-accent-dim text-shell-accent"
                  : "text-shell-muted hover:bg-shell-hover hover:text-shell-text",
              ].join(" ")}
            >
              {t(f.labelKey)}
            </button>
          ))}
        </div>

        <section className="overflow-hidden rounded-xl border border-shell-border/80 bg-shell-elevated">
          {filtered.length === 0 ? (
            <div className="px-5 py-16 text-center text-sm text-shell-muted">
              {t("downloadsMgr.empty")}
            </div>
          ) : (
            <ul className="divide-y divide-shell-border/70">
              {filtered.map((item) => (
                <li key={item.id} className="flex items-start gap-4 px-5 py-4">
                  <div className="mt-0.5 flex size-10 shrink-0 items-center justify-center rounded-lg bg-shell-hover text-shell-accent">
                    <IconFolder size={18} stroke={1.6} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] text-shell-text">
                      {item.name}
                    </div>
                    <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-shell-muted">
                      <StatusBadge item={item} />
                      <span>·</span>
                      <span>{item.size}</span>
                      <span>·</span>
                      <span>{item.source}</span>
                      <span>·</span>
                      <span>{t(item.timeKey)}</span>
                    </div>
                    {item.progress != null && item.status !== "complete" ? (
                      <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-shell-track/50">
                        <div
                          className="h-full rounded-full bg-shell-accent"
                          style={{ width: `${item.progress}%` }}
                        />
                      </div>
                    ) : null}
                    <div className="mt-1 truncate text-[11px] text-shell-muted">
                      {item.path}
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    {item.status === "paused" || item.status === "failed" ? (
                      <button
                        type="button"
                        className="rounded-full p-1.5 text-shell-muted hover:bg-shell-hover hover:text-shell-text"
                        aria-label={t("downloadsMgr.actions.resume")}
                        onClick={() =>
                          setItems((prev) =>
                            prev.map((d) =>
                              d.id === item.id
                                ? { ...d, status: "progress", progress: d.progress ?? 10 }
                                : d,
                            ),
                          )
                        }
                      >
                        <IconPlayerPlay size={16} stroke={1.6} />
                      </button>
                    ) : null}
                    <button
                      type="button"
                      className="rounded-full p-1.5 text-shell-muted hover:bg-shell-hover hover:text-shell-danger"
                      aria-label={t("downloadsMgr.actions.remove")}
                      onClick={() => remove(item.id)}
                    >
                      {item.status === "complete" ? (
                        <IconTrash size={16} stroke={1.6} />
                      ) : (
                        <IconX size={16} stroke={1.6} />
                      )}
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
