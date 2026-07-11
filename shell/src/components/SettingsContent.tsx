import { useTranslation } from "react-i18next";
import { SettingRowView } from "./SettingRowView";
import { LanguageSwitcher } from "./LanguageSwitcher";
import { AboutPanel } from "./AboutPanel";
import type { NavId, SettingSection } from "../data/settings";

type Props = {
  page: NavId;
  sections: SettingSection[];
  onToggle: (sectionId: string, rowId: string, on: boolean) => void;
};

export function SettingsContent({ page, sections, onToggle }: Props) {
  const { t } = useTranslation();
  const heading = sections[0];

  return (
    <div className="mx-auto w-full max-w-[640px] px-7 py-8">
      {heading ? (
        <header className="mb-6">
          <h1 className="page-title">{t(heading.titleKey)}</h1>
          {heading.descriptionKey ? (
            <p className="page-sub max-w-[480px]">{t(heading.descriptionKey)}</p>
          ) : null}
        </header>
      ) : null}

      <div className="flex flex-col gap-3.5">
        {page === "languages" ? <LanguageSwitcher /> : null}
        {page === "about" ? <AboutPanel /> : null}

        {sections.map((section) => (
          <section key={section.id} className="panel">
            {section.id !== heading?.id ? (
              <div className="border-b border-shell-border/50 px-5 py-3 text-[13px] font-medium tracking-[-0.01em] text-shell-text">
                {t(section.titleKey)}
              </div>
            ) : null}
            <div className="divide-y divide-shell-border/50">
              {section.rows.map((row) => (
                <SettingRowView
                  key={row.id}
                  page={page}
                  row={row}
                  onToggle={(rowId, on) => onToggle(section.id, rowId, on)}
                />
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
