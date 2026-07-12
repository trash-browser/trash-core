import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { ShellIcon } from "./ShellIcon";
import { Toggle } from "./Toggle";
import type { SettingRow } from "../data/settings";
import type { NavId } from "../data/settings";
import {
  chooseDownloadLocation,
  getDownloadPreferences,
} from "../native/ipc";

type Props = {
  page: NavId;
  row: SettingRow;
  onToggle: (id: string, on: boolean) => void;
};

export function SettingRowView({ page, row, onToggle }: Props) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const [downloadLocation, setDownloadLocation] = useState("");
  const [choosingLocation, setChoosingLocation] = useState(false);
  const title = t(row.titleKey);
  const description =
    page === "downloads" && row.id === "location" && downloadLocation
      ? downloadLocation
      :
    page === "languages" && row.id === "preferred"
      ? t(`langs.${(i18n.resolvedLanguage || "en").slice(0, 2)}`)
      : row.descriptionKey
        ? t(row.descriptionKey)
        : undefined;
  const href = `/settings/${page}/${row.id}`;

  useEffect(() => {
    if (page !== "downloads" || row.id !== "location") return;
    let cancelled = false;
    const load = () => void getDownloadPreferences().then((preferences) => {
      if (!cancelled) setDownloadLocation(preferences.location);
    });
    load();
    // The native folder picker resolves asynchronously via this event, carrying
    // the outcome: a new location, a cancellation, or a dialog error.
    const changed = (event: Event) => {
      const detail = (event as CustomEvent<{
        location?: string;
        cancelled?: boolean;
        error?: string;
      }>).detail;
      // Any outcome ends the active request; re-enable the row.
      setChoosingLocation(false);
      if (detail?.error) {
        console.error("Failed to choose download location", detail.error);
        return;
      }
      if (detail?.cancelled) return; // user dismissed — leave the location as-is
      if (detail?.location) setDownloadLocation(detail.location);
    };
    window.addEventListener("trash-download-preferences", changed);
    window.addEventListener("trash-cef-ready", load);
    return () => {
      cancelled = true;
      window.removeEventListener("trash-download-preferences", changed);
      window.removeEventListener("trash-cef-ready", load);
    };
  }, [page, row.id]);

  const chooseLocation = async () => {
    // Debounce: ignore clicks while a picker request is already in flight so we
    // never open (or enqueue) two dialogs at once.
    if (choosingLocation) return;
    setChoosingLocation(true);
    try {
      const opened = await chooseDownloadLocation();
      // On success the result arrives via 'trash-download-preferences', which
      // clears the flag. If the host rejected/is unavailable (e.g. Vite dev),
      // no event fires — reset here so the row is not stuck disabled.
      if (!opened) setChoosingLocation(false);
    } catch (error) {
      console.error("Failed to choose download location", error);
      setChoosingLocation(false);
    }
  };

  const go = () => {
    if (page === "downloads" && row.id === "location") {
      void chooseLocation();
      return;
    }
    navigate(href);
  };

  return (
    <div
      className="group flex cursor-pointer items-center gap-4 px-5 py-3.5 hover:bg-shell-hover/60"
      role="link"
      tabIndex={0}
      aria-busy={choosingLocation || undefined}
      onClick={go}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          go();
        }
      }}
    >
      <div className="min-w-0 flex-1">
        <div className="text-[13px] leading-5 text-shell-text">{title}</div>
        {description ? (
          <div className="mt-0.5 text-xs leading-4 text-shell-muted">{description}</div>
        ) : null}
      </div>

      {row.control.kind === "toggle" ? (
        <div
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          <Toggle
            on={row.control.on}
            label={title}
            onChange={(next) => onToggle(row.id, next)}
          />
        </div>
      ) : null}

      {row.control.kind === "select" ? (
        <div className="flex items-center gap-1 text-[13px] text-shell-muted">
          <span className="max-w-[220px] truncate">{t(row.control.valueKey)}</span>
          <ShellIcon name="chevron" size={16} className="text-shell-muted" />
        </div>
      ) : null}

      {row.control.kind === "link" ? (
        <div className="flex items-center gap-2 text-[13px] text-shell-muted">
          {row.control.valueKey ? (
            <span className="text-shell-accent">{t(row.control.valueKey)}</span>
          ) : (
            <ShellIcon name="chevron" size={16} />
          )}
        </div>
      ) : null}
    </div>
  );
}
