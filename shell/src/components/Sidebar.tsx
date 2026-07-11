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
    <aside className="flex h-full w-[232px] shrink-0 flex-col border-r border-shell-border/60 bg-shell">
      <div className="px-5 pt-6 pb-3">
        <div className="text-[15px] font-medium tracking-[-0.02em] text-shell-text">
          {t("shell.settings")}
        </div>
        <div className="mt-1 text-[11px] tracking-wide text-shell-muted">
          {t("shell.product")}
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto px-2.5 pb-4">
        <ul className="flex flex-col gap-0.5">
          {NAV.map((item) => {
            const isActive = item.id === active;
            return (
              <li key={item.id}>
                <NavLink
                  to={`/settings/${item.id}`}
                  end={false}
                  className={[
                    "flex w-full items-center gap-2.5 rounded-xl px-3 py-2 text-left text-[13px] tracking-[-0.01em] transition-colors",
                    isActive
                      ? "bg-shell-active text-shell-text"
                      : "text-shell-muted hover:bg-shell-hover hover:text-shell-text",
                  ].join(" ")}
                >
                  <ShellIcon
                    name={item.icon}
                    size={16}
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
