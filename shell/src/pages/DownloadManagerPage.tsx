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
      <div className="mx-auto w-full max-w-[720px] px-7 py-8">
        <header className="mb-6 flex flex-wrap items-end justify-between gap-3">
          <div>
            <h1 className="page-title">{t("downloadsMgr.title")}</h1>
            <p className="page-sub">{t("downloadsMgr.subtitle")}</p>
          </div>
          <button
            type="button"
            onClick={clearCompleted}
            className="rounded-xl px-3 py-1.5 text-[12px] text-shell-muted hover:bg-shell-hover hover:text-shell-text"
          >
            {t("downloadsMgr.clearCompleted")}
          </button>
        </header>

        <label className="mb-3.5 flex h-10 items-center gap-2.5 rounded-xl border border-shell-border/80 bg-shell-elevated px-3.5 transition-[border-color] focus-within:border-shell-accent/60">
          <IconSearch size={15} stroke={1.5} className="text-shell-muted" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("downloadsMgr.search")}
            className="h-full w-full bg-transparent text-[13px] tracking-[-0.01em] text-shell-text outline-none placeholder:text-shell-muted"
          />
        </label>

        <div className="mb-3.5 flex flex-wrap gap-1">
          {filters.map((f) => (
            <button
              key={f.id}
              type="button"
              onClick={() => setFilter(f.id)}
              className={[
                "rounded-xl px-3 py-1.5 text-[12px] transition-colors",
                filter === f.id
                  ? "bg-shell-active text-shell-text"
                  : "text-shell-muted hover:bg-shell-hover hover:text-shell-text",
              ].join(" ")}
            >
              {t(f.labelKey)}
            </button>
          ))}
        </div>

        <section className="panel">
          {filtered.length === 0 ? (
            <div className="px-5 py-16 text-center text-[13px] text-shell-muted">
              {t("downloadsMgr.empty")}
            </div>
          ) : (
            <ul className="divide-y divide-shell-border/50">
              {filtered.map((item) => (
                <li key={item.id} className="flex items-start gap-3.5 px-5 py-4">
                  <div className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-xl bg-shell-hover text-shell-muted">
                    <IconFolder size={16} stroke={1.5} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] tracking-[-0.01em] text-shell-text">
                      {item.name}
                    </div>
                    <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[12px] text-shell-muted">
                      <StatusBadge item={item} />
                      <span>·</span>
                      <span>{item.size}</span>
                      <span>·</span>
                      <span>{item.source}</span>
                      <span>·</span>
                      <span>{t(item.timeKey)}</span>
                    </div>
                    {item.progress != null && item.status !== "complete" ? (
                      <div className="mt-2.5 h-1.5 overflow-hidden rounded-full bg-shell-track/35">
                        <div
                          className="h-full rounded-full bg-shell-accent/85"
                          style={{ width: `${item.progress}%` }}
                        />
                      </div>
                    ) : null}
                    <div className="mt-1 truncate text-[11px] text-shell-muted">
                      {item.path}
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-0.5">
                    {item.status === "paused" || item.status === "failed" ? (
                      <button
                        type="button"
                        className="rounded-xl p-1.5 text-shell-muted hover:bg-shell-hover hover:text-shell-text"
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
                        <IconPlayerPlay size={15} stroke={1.5} />
                      </button>
                    ) : null}
                    <button
                      type="button"
                      className="rounded-xl p-1.5 text-shell-muted hover:bg-shell-hover hover:text-shell-danger"
                      aria-label={t("downloadsMgr.actions.remove")}
                      onClick={() => remove(item.id)}
                    >
                      {item.status === "complete" ? (
                        <IconTrash size={15} stroke={1.5} />
                      ) : (
                        <IconX size={15} stroke={1.5} />
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
