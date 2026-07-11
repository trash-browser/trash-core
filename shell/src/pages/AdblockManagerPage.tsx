import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { IconPlus, IconRefresh, IconShieldOff } from "@tabler/icons-react";
import { MOCK_FILTER_LISTS } from "../data/managers";
import { Toggle } from "../components/Toggle";

export function AdblockManagerPage() {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState(true);
  const [cosmetic, setCosmetic] = useState(true);
  const [lists, setLists] = useState(MOCK_FILTER_LISTS);
  const [customRule, setCustomRule] = useState("");
  const [customRules, setCustomRules] = useState<string[]>([
    "||tracker.example^",
    "@@||trusted.partner.example^",
  ]);

  const activeRules = useMemo(() => {
    const fromLists = lists
      .filter((l) => l.enabled)
      .reduce((sum, l) => sum + Number(l.rules.replace(/,/g, "")), 0);
    return fromLists + customRules.length;
  }, [lists, customRules]);

  const toggleList = (id: string, on: boolean) =>
    setLists((prev) =>
      prev.map((list) => (list.id === id ? { ...list, enabled: on } : list)),
    );

  const addRule = () => {
    const rule = customRule.trim();
    if (!rule) return;
    setCustomRules((prev) => [rule, ...prev]);
    setCustomRule("");
  };

  return (
    <div className="flex h-full min-h-0 flex-col overflow-y-auto bg-shell">
      <div className="mx-auto w-full max-w-[860px] px-6 py-8">
        <header className="mb-6">
          <div className="mb-3 flex size-12 items-center justify-center rounded-2xl bg-shell-accent-dim text-shell-accent">
            <IconShieldOff size={24} stroke={1.6} />
          </div>
          <h1 className="text-[28px] font-normal tracking-tight text-shell-text">
            {t("adblock.title")}
          </h1>
          <p className="mt-1 text-sm text-shell-muted">{t("adblock.subtitle")}</p>
        </header>

        <div className="mb-4 grid gap-4 sm:grid-cols-3">
          <div className="rounded-xl border border-shell-border/80 bg-shell-elevated px-5 py-4">
            <div className="text-xs text-shell-muted">{t("adblock.stats.blocked")}</div>
            <div className="mt-1 text-2xl text-shell-text">128,440</div>
          </div>
          <div className="rounded-xl border border-shell-border/80 bg-shell-elevated px-5 py-4">
            <div className="text-xs text-shell-muted">{t("adblock.stats.rules")}</div>
            <div className="mt-1 text-2xl text-shell-text">
              {activeRules.toLocaleString()}
            </div>
          </div>
          <div className="rounded-xl border border-shell-border/80 bg-shell-elevated px-5 py-4">
            <div className="text-xs text-shell-muted">{t("adblock.stats.lists")}</div>
            <div className="mt-1 text-2xl text-shell-text">
              {lists.filter((l) => l.enabled).length}/{lists.length}
            </div>
          </div>
        </div>

        <section className="mb-4 overflow-hidden rounded-xl border border-shell-border/80 bg-shell-elevated">
          <div className="flex items-center gap-4 px-5 py-3.5">
            <div className="min-w-0 flex-1">
              <div className="text-[13px] text-shell-text">
                {t("adblock.enable.title")}
              </div>
              <div className="mt-0.5 text-xs text-shell-muted">
                {t("adblock.enable.description")}
              </div>
            </div>
            <Toggle
              on={enabled}
              label={t("adblock.enable.title")}
              onChange={setEnabled}
            />
          </div>
          <div className="flex items-center gap-4 border-t border-shell-border/70 px-5 py-3.5">
            <div className="min-w-0 flex-1">
              <div className="text-[13px] text-shell-text">
                {t("adblock.cosmetic.title")}
              </div>
              <div className="mt-0.5 text-xs text-shell-muted">
                {t("adblock.cosmetic.description")}
              </div>
            </div>
            <Toggle
              on={cosmetic && enabled}
              label={t("adblock.cosmetic.title")}
              onChange={setCosmetic}
            />
          </div>
        </section>

        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm text-shell-text">{t("adblock.listsTitle")}</h2>
          <button
            type="button"
            className="inline-flex items-center gap-1 rounded-full px-3 py-1.5 text-[12px] text-shell-accent hover:bg-shell-accent-dim"
          >
            <IconRefresh size={14} stroke={1.6} />
            {t("adblock.updateLists")}
          </button>
        </div>

        <section className="mb-4 overflow-hidden rounded-xl border border-shell-border/80 bg-shell-elevated">
          <ul className="divide-y divide-shell-border/70">
            {lists.map((list) => (
              <li key={list.id} className="flex items-start gap-4 px-5 py-3.5">
                <div className="min-w-0 flex-1">
                  <div className="text-[13px] text-shell-text">{t(list.nameKey)}</div>
                  <div className="mt-0.5 text-xs text-shell-muted">
                    {t(list.descKey)}
                  </div>
                  <div className="mt-1 text-[11px] text-shell-muted">
                    {t("adblock.rulesCount", { count: list.rules })} ·{" "}
                    {t(list.updatedKey)}
                  </div>
                </div>
                <Toggle
                  on={list.enabled && enabled}
                  label={t(list.nameKey)}
                  onChange={(on) => toggleList(list.id, on)}
                />
              </li>
            ))}
          </ul>
        </section>

        <h2 className="mb-2 text-sm text-shell-text">{t("adblock.customTitle")}</h2>
        <section className="overflow-hidden rounded-xl border border-shell-border/80 bg-shell-elevated">
          <div className="flex gap-2 border-b border-shell-border/70 px-4 py-3">
            <input
              value={customRule}
              onChange={(e) => setCustomRule(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") addRule();
              }}
              placeholder={t("adblock.customPlaceholder")}
              className="h-9 min-w-0 flex-1 rounded-lg border border-shell-border bg-shell px-3 text-[13px] text-shell-text outline-none placeholder:text-shell-muted focus:border-shell-accent"
            />
            <button
              type="button"
              onClick={addRule}
              className="inline-flex items-center gap-1 rounded-lg bg-shell-accent-dim px-3 text-[13px] text-shell-accent hover:bg-shell-hover"
            >
              <IconPlus size={16} stroke={1.6} />
              {t("adblock.addRule")}
            </button>
          </div>
          <ul className="divide-y divide-shell-border/70">
            {customRules.map((rule) => (
              <li
                key={rule}
                className="flex items-center gap-3 px-5 py-3 font-mono text-[12px] text-shell-text"
              >
                <span className="min-w-0 flex-1 truncate">{rule}</span>
                <button
                  type="button"
                  className="text-xs text-shell-muted hover:text-shell-danger"
                  onClick={() =>
                    setCustomRules((prev) => prev.filter((r) => r !== rule))
                  }
                >
                  {t("adblock.remove")}
                </button>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </div>
  );
}
