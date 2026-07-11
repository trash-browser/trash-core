import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { ShellIcon } from "./ShellIcon";
import { Toggle } from "./Toggle";
import type { SettingRow } from "../data/settings";
import type { NavId } from "../data/settings";

type Props = {
  page: NavId;
  row: SettingRow;
  onToggle: (id: string, on: boolean) => void;
};

export function SettingRowView({ page, row, onToggle }: Props) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const title = t(row.titleKey);
  const description =
    page === "languages" && row.id === "preferred"
      ? t(`langs.${(i18n.resolvedLanguage || "en").slice(0, 2)}`)
      : row.descriptionKey
        ? t(row.descriptionKey)
        : undefined;
  const href = `/settings/${page}/${row.id}`;

  const go = () => navigate(href);

  return (
    <div
      className="group flex cursor-pointer items-center gap-4 px-5 py-3.5 hover:bg-shell-hover/60"
      role="link"
      tabIndex={0}
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
