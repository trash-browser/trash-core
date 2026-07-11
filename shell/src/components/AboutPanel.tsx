import { useTranslation } from "react-i18next";
import { motion } from "framer-motion";
import { IconExternalLink, IconInfoCircle, IconSettings } from "@tabler/icons-react";
import { VERSIONS } from "../data/version";
import { listItemVariants } from "../motion";

/**
 * Navigate the CEF content browser to Chromium's built-in settings page. This
 * leaves the miko:// shell and loads chrome://settings in the same tab, just
 * like typing it into the omnibox.
 */
function openChromeSettings() {
  window.location.href = "chrome://settings";
}

export function AboutPanel() {
  const { t } = useTranslation();

  const rows: { label: string; value: string }[] = [
    { label: t("about.appVersion"), value: VERSIONS.app },
    { label: t("about.cefVersion"), value: VERSIONS.cef },
    { label: t("about.chromiumVersion"), value: VERSIONS.chromium },
  ];

  return (
    <motion.section
      variants={listItemVariants}
      className="overflow-hidden rounded-xl border border-shell-border/80 bg-shell-elevated"
    >
      <div className="flex items-center gap-4 px-5 py-4">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-shell-accent-dim text-shell-accent">
          <IconInfoCircle size={22} stroke={1.6} />
        </div>
        <div className="min-w-0">
          <div className="text-[15px] leading-5 text-shell-text">
            {t("shell.product")}
          </div>
          <div className="mt-0.5 text-xs leading-4 text-shell-muted">
            {t("about.tagline")}
          </div>
        </div>
      </div>

      <div className="divide-y divide-shell-border/70 border-t border-shell-border/80">
        {rows.map((row) => (
          <div
            key={row.label}
            className="flex items-center justify-between gap-4 px-5 py-3"
          >
            <span className="text-[13px] leading-5 text-shell-text">
              {row.label}
            </span>
            <span className="max-w-[60%] truncate font-mono text-xs text-shell-muted [user-select:text]">
              {row.value}
            </span>
          </div>
        ))}
      </div>

      <div className="border-t border-shell-border/80 px-5 py-4">
        <button
          type="button"
          onClick={openChromeSettings}
          className="inline-flex items-center gap-2 rounded-lg border border-shell-border bg-shell-hover/40 px-3.5 py-2 text-[13px] text-shell-text transition-colors hover:bg-shell-hover"
        >
          <IconSettings size={16} stroke={1.6} className="text-shell-muted" />
          {t("about.openChromeSettings")}
          <IconExternalLink size={14} stroke={1.6} className="text-shell-muted" />
        </button>
      </div>
    </motion.section>
  );
}
