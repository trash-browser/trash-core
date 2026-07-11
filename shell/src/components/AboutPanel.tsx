import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { motion } from "framer-motion";
import { IconExternalLink, IconSettings } from "@tabler/icons-react";
import { VERSIONS as BUILD_VERSIONS } from "../data/version";
import { getVersions, type Versions } from "../native/ipc";
import { listItemVariants } from "../motion";
import appIcon from "../assets/app-icon.png";

/**
 * Navigate the CEF content browser to Chromium's built-in settings page. This
 * leaves the miko:// shell and loads chrome://settings in the same tab, just
 * like typing it into the omnibox.
 */
function openChromeSettings() {
  void window.trashCef?.navigate?.("chrome://settings");
  window.location.href = "chrome://settings";
}

export function AboutPanel() {
  const { t } = useTranslation();
  const [versions, setVersions] = useState<Versions>(BUILD_VERSIONS);

  useEffect(() => {
    let cancelled = false;
    void getVersions().then((live) => {
      if (!cancelled && live.app !== "dev") {
        setVersions(live);
      } else if (!cancelled) {
        setVersions({
          app: live.app || BUILD_VERSIONS.app,
          cef: live.cef !== "unknown" ? live.cef : BUILD_VERSIONS.cef,
          chromium:
            live.chromium !== "unknown"
              ? live.chromium
              : BUILD_VERSIONS.chromium,
        });
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const rows: { label: string; value: string }[] = [
    { label: t("about.appVersion"), value: versions.app },
    { label: t("about.cefVersion"), value: versions.cef },
    { label: t("about.chromiumVersion"), value: versions.chromium },
  ];

  return (
    <motion.section variants={listItemVariants} className="panel">
      <div className="flex items-center gap-4 px-5 py-4">
        <img
          src={appIcon}
          alt=""
          width={44}
          height={44}
          draggable={false}
          className="size-11 shrink-0 rounded-[12px] shadow-[0_8px_20px_-10px_rgb(0_0_0/0.6)]"
        />
        <div className="min-w-0">
          <div className="text-[15px] font-medium tracking-[-0.02em] text-shell-text">
            {t("shell.product")}
          </div>
          <div className="mt-0.5 text-xs leading-4 text-shell-muted">
            {t("about.tagline")}
          </div>
        </div>
      </div>

      <div className="divide-y divide-shell-border/50 border-t border-shell-border/50">
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

      <div className="border-t border-shell-border/50 px-5 py-4">
        <button
          type="button"
          onClick={openChromeSettings}
          className="inline-flex items-center gap-2 rounded-xl border border-shell-border/80 bg-shell-hover/30 px-3.5 py-2 text-[13px] text-shell-text transition-colors hover:bg-shell-hover"
        >
          <IconSettings size={16} stroke={1.6} className="text-shell-muted" />
          {t("about.openChromeSettings")}
          <IconExternalLink size={14} stroke={1.6} className="text-shell-muted" />
        </button>
      </div>
    </motion.section>
  );
}
