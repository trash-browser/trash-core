import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { ShellIcon } from "./ShellIcon";
import { NAV, type NavId } from "../data/settings";

type Props = {
  active: NavId;
};

export function Sidebar({ active }: Props) {
  const { t } = useTranslation();

  return (
    <aside className="flex h-full w-[256px] shrink-0 flex-col border-r border-shell-border/80 bg-shell">
      <div className="px-5 pt-6 pb-3">
        <div className="text-lg font-medium tracking-tight text-shell-text">
          {t("shell.settings")}
        </div>
        <div className="mt-0.5 text-xs text-shell-muted">{t("shell.product")}</div>
      </div>

      <nav className="flex-1 overflow-y-auto px-2 pb-4">
        <ul className="flex flex-col gap-0.5">
          {NAV.map((item) => {
            const isActive = item.id === active;
            return (
              <li key={item.id}>
                <NavLink
                  to={`/settings/${item.id}`}
                  end={false}
                  className={[
                    "flex w-full items-center gap-3 rounded-full px-3 py-2 text-left text-[13px] transition-colors",
                    isActive
                      ? "bg-shell-accent-dim text-shell-accent"
                      : "text-shell-text hover:bg-shell-hover",
                  ].join(" ")}
                >
                  <ShellIcon
                    name={item.icon}
                    size={18}
                    className={isActive ? "text-shell-accent" : "text-shell-muted"}
                  />
                  <span className="truncate">{t(item.labelKey)}</span>
                </NavLink>
              </li>
            );
          })}
        </ul>
      </nav>
    </aside>
  );
}
