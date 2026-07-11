import { useMemo, useState } from "react";
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
