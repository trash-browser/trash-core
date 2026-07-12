import { useEffect, useMemo, useState } from "react";
import { Navigate, Outlet, useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Sidebar } from "../components/Sidebar";
import { SettingsSearch } from "../components/SettingsSearch";
import { SettingsContent } from "../components/SettingsContent";
import {
  DEFAULT_NAV,
  PAGES,
  isNavId,
  type NavId,
  type SettingSection,
} from "../data/settings";
import { getSettings, setSetting } from "../native/ipc";

const settingKey = (page: NavId, rowId: string) => `settings.${page}.${rowId}`;

function withPersistedToggles(
  pages: typeof PAGES,
  values: Record<string, boolean | string | number>,
): typeof PAGES {
  return Object.fromEntries(
    Object.entries(pages).map(([page, sections]) => [
      page,
      sections.map((section) => ({
        ...section,
        rows: section.rows.map((row) => {
          const value = values[settingKey(page as NavId, row.id)];
          if (row.control.kind !== "toggle" || typeof value !== "boolean") return row;
          return { ...row, control: { kind: "toggle" as const, on: value } };
        }),
      })),
    ]),
  ) as typeof PAGES;
}

function filterSections(
  sections: SettingSection[],
  query: string,
  t: (key: string) => string,
): SettingSection[] {
  const q = query.trim().toLowerCase();
  if (!q) return sections;
  return sections
    .map((section) => ({
      ...section,
      rows: section.rows.filter((row) => {
        const title = t(row.titleKey).toLowerCase();
        const description = row.descriptionKey
          ? t(row.descriptionKey).toLowerCase()
          : "";
        return title.includes(q) || description.includes(q);
      }),
    }))
    .filter((section) => section.rows.length > 0);
}

export function SettingsShell() {
  const { page } = useParams();

  if (!isNavId(page)) {
    return <Navigate to={`/settings/${DEFAULT_NAV}`} replace />;
  }

  return (
    <div className="flex h-full min-h-0 bg-shell text-shell-text">
      <Sidebar active={page} />
      <main className="flex min-w-0 flex-1 flex-col">
        <div className="min-h-0 flex-1 overflow-y-auto">
          <Outlet />
        </div>
      </main>
    </div>
  );
}

export function SettingsListPage() {
  const { page } = useParams();
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState("");
  const [pages, setPages] = useState(PAGES);

  useEffect(() => {
    let cancelled = false;
    void getSettings().then((values) => {
      if (!cancelled) setPages((current) => withPersistedToggles(current, values));
    });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!isNavId(page)) {
    return <Navigate to={`/settings/${DEFAULT_NAV}`} replace />;
  }

  const active: NavId = page;
  const sections = useMemo(
    () => filterSections(pages[active], query, t),
    [pages, active, query, t, i18n.language],
  );

  const onToggle = (sectionId: string, rowId: string, on: boolean) => {
    setPages((prev) => {
      const next = { ...prev };
      next[active] = prev[active].map((section) => {
        if (section.id !== sectionId) return section;
        return {
          ...section,
          rows: section.rows.map((row) => {
            if (row.id !== rowId || row.control.kind !== "toggle") return row;
            return { ...row, control: { kind: "toggle", on } };
          }),
        };
      });
      return next;
    });
    void setSetting(settingKey(active, rowId), on).then((saved) => {
      if (!saved) {
        setPages((current) =>
          withPersistedToggles(current, { [settingKey(active, rowId)]: !on }),
        );
      }
    });
  };

  return (
    <>
      <div className="sticky top-0 z-10">
        <SettingsSearch value={query} onChange={setQuery} />
      </div>
      {sections.length ? (
        <SettingsContent page={active} sections={sections} onToggle={onToggle} />
      ) : (
        <div className="mx-auto max-w-[680px] px-6 py-16 text-center text-sm text-shell-muted">
          {t("shell.noMatch", { query })}
        </div>
      )}
    </>
  );
}
