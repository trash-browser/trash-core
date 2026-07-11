import { useTranslation } from "react-i18next";
import { motion } from "framer-motion";
import { SettingRowView } from "./SettingRowView";
import { LanguageSwitcher } from "./LanguageSwitcher";
import { AboutPanel } from "./AboutPanel";
import type { NavId, SettingSection } from "../data/settings";
import { listContainerVariants, listItemVariants } from "../motion";

type Props = {
  page: NavId;
  sections: SettingSection[];
  onToggle: (sectionId: string, rowId: string, on: boolean) => void;
};

export function SettingsContent({ page, sections, onToggle }: Props) {
  const { t } = useTranslation();
  const heading = sections[0];

  return (
    <div className="mx-auto w-full max-w-[680px] px-6 py-8">
      {heading ? (
        <motion.header
          className="mb-6"
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
        >
          <h1 className="text-[28px] font-normal tracking-tight text-shell-text">
            {t(heading.titleKey)}
          </h1>
          {heading.descriptionKey ? (
            <p className="mt-2 max-w-[540px] text-sm leading-5 text-shell-muted">
              {t(heading.descriptionKey)}
            </p>
          ) : null}
        </motion.header>
      ) : null}

      <motion.div
        className="flex flex-col gap-4"
        variants={listContainerVariants}
        initial="initial"
        animate="animate"
      >
        {page === "languages" ? <LanguageSwitcher /> : null}
        {page === "about" ? <AboutPanel /> : null}

        {sections.map((section) => (
          <motion.section
            key={section.id}
            variants={listItemVariants}
            className="overflow-hidden rounded-xl border border-shell-border/80 bg-shell-elevated"
          >
            {section.id !== heading?.id ? (
              <div className="border-b border-shell-border/80 px-5 py-3 text-sm text-shell-text">
                {t(section.titleKey)}
              </div>
            ) : null}
            <div className="divide-y divide-shell-border/70">
              {section.rows.map((row) => (
                <SettingRowView
                  key={row.id}
                  page={page}
                  row={row}
                  onToggle={(rowId, on) => onToggle(section.id, rowId, on)}
                />
              ))}
            </div>
          </motion.section>
        ))}
      </motion.div>
    </div>
  );
}
