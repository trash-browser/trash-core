import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { motion } from "framer-motion";
import { IconPlus, IconSearch } from "@tabler/icons-react";
import { Link } from "react-router-dom";
import { NTP_SHORTCUTS } from "../data/managers";
import {
  useAdblockFilterStatus,
  useAdblockStatistics,
} from "../adblockStats";
import { navigate, resolveSearchQuery } from "../native/ipc";
import appIcon from "../assets/app-icon.png";

const easeOut = [0.22, 1, 0.36, 1] as const;

export function NewTabPage() {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const statistics = useAdblockStatistics();
  const filterStatus = useAdblockFilterStatus();

  const statusLine =
    filterStatus.state === "active"
      ? t("ntp.protection.active", { count: filterStatus.listsLoaded })
      : filterStatus.state === "error"
        ? t("ntp.protection.error")
        : t("ntp.protection.loading");

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    const url = resolveSearchQuery(query);
    if (!url) return;
    void navigate(url);
  };

  return (
    <div className="ntp-canvas flex h-full min-h-0 flex-col overflow-y-auto">
      <div className="mx-auto flex w-full max-w-[560px] flex-1 flex-col px-6 pt-[14vh] pb-10">
        <motion.div
          className="mb-8 flex select-none flex-col items-center gap-3"
          initial={{ opacity: 0, y: 10, scale: 0.96 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          transition={{ duration: 0.42, ease: easeOut }}
        >
          <img
            src={appIcon}
            alt=""
            width={72}
            height={72}
            draggable={false}
            className="ntp-logo size-[72px] rounded-[18px]"
          />
          <div className="text-[15px] font-medium tracking-[0.18em] text-shell-muted/90 lowercase">
            trash
          </div>
        </motion.div>

        <motion.form
          onSubmit={onSubmit}
          className="w-full"
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.4, delay: 0.06, ease: easeOut }}
        >
          <label className="ntp-search flex h-12 items-center gap-3 rounded-2xl border border-shell-border/90 bg-shell-elevated/90 px-4 backdrop-blur-sm">
            <IconSearch size={18} stroke={1.5} className="shrink-0 text-shell-muted" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("ntp.searchPlaceholder")}
              autoFocus
              className="h-full w-full bg-transparent text-[15px] tracking-[-0.01em] text-shell-text outline-none placeholder:text-shell-muted/75"
            />
          </label>
        </motion.form>

        <motion.div
          className="mt-11 grid grid-cols-4 gap-x-3 gap-y-5 sm:grid-cols-4"
          initial="hidden"
          animate="show"
          variants={{
            hidden: {},
            show: {
              transition: { staggerChildren: 0.035, delayChildren: 0.12 },
            },
          }}
        >
          {NTP_SHORTCUTS.map((item) => (
            <motion.a
              key={item.id}
              href={item.url}
              onClick={(e) => {
                e.preventDefault();
                void navigate(item.url);
              }}
              className="group flex flex-col items-center gap-2.5"
              variants={{
                hidden: { opacity: 0, y: 10 },
                show: {
                  opacity: 1,
                  y: 0,
                  transition: { duration: 0.32, ease: easeOut },
                },
              }}
            >
              <span
                className="ntp-tile flex size-[52px] items-center justify-center rounded-[15px] text-[15px] font-medium tracking-tight text-shell-text/95 shadow-[0_1px_0_rgb(255_255_255/0.06)_inset] ring-1 ring-inset ring-white/[0.06]"
                style={{ background: item.tint }}
              >
                {item.letter}
              </span>
              <span className="max-w-[88px] truncate text-center text-[12px] tracking-[-0.01em] text-shell-muted transition-colors duration-150 group-hover:text-shell-text">
                {t(item.labelKey)}
              </span>
            </motion.a>
          ))}
          <motion.button
            type="button"
            className="group flex flex-col items-center gap-2.5"
            aria-label={t("ntp.addShortcut")}
            variants={{
              hidden: { opacity: 0, y: 10 },
              show: {
                opacity: 1,
                y: 0,
                transition: { duration: 0.32, ease: easeOut },
              },
            }}
          >
            <span className="ntp-tile flex size-[52px] items-center justify-center rounded-[15px] border border-dashed border-shell-border/90 text-shell-muted group-hover:border-shell-muted group-hover:bg-shell-hover/40 group-hover:text-shell-text">
              <IconPlus size={18} stroke={1.5} />
            </span>
            <span className="text-[12px] text-shell-muted">{t("ntp.addShortcut")}</span>
          </motion.button>
        </motion.div>

        <motion.div
          className="mt-auto flex justify-center pt-16"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.35, delay: 0.28, ease: easeOut }}
        >
          <Link
            to="/adblock"
            className="rounded-full px-3 py-1 text-[11px] tracking-wide text-shell-muted/90 transition-colors duration-150 hover:bg-shell-hover/50 hover:text-shell-text"
            aria-live="polite"
          >
            <span className="tabular-nums">
              {statistics.totalBlocked.toLocaleString()}
            </span>
            <span className="mx-1.5 text-shell-border">·</span>
            {statusLine}
          </Link>
        </motion.div>
      </div>
    </div>
  );
}
