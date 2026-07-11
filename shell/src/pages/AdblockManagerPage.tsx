import { useState } from "react";
import { useTranslation } from "react-i18next";
import { IconPlus, IconRefresh } from "@tabler/icons-react";
import { MOCK_FILTER_LISTS } from "../data/managers";
import { Toggle } from "../components/Toggle";
import { useAdblockStatistics } from "../adblockStats";

function resourceTypeLabel(resourceType: string) {
  const words = resourceType.replace(/([a-z])([A-Z])/g, "$1 $2");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

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
  const statistics = useAdblockStatistics();
  const largestResourceCount = Math.max(
    1,
    ...statistics.byResource.map((resource) => resource.total),
  );

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
      <div className="mx-auto w-full max-w-[720px] px-7 py-8">
        <header className="mb-6">
          <h1 className="page-title">{t("adblock.title")}</h1>
          <p className="page-sub">{t("adblock.subtitle")}</p>
        </header>

        <div className="mb-6 flex flex-wrap gap-x-10 gap-y-3 border-b border-shell-border/50 pb-5">
          <div>
            <div className="text-[11px] tracking-wide text-shell-muted">
              {t("adblock.stats.blocked")}
            </div>
            <div className="mt-1 tabular-nums text-[22px] tracking-[-0.03em] text-shell-text">
              {statistics.totalBlocked.toLocaleString()}
            </div>
          </div>
          <div>
            <div className="text-[11px] tracking-wide text-shell-muted">
              {t("adblock.stats.today")}
            </div>
            <div className="mt-1 tabular-nums text-[22px] tracking-[-0.03em] text-shell-text">
              {statistics.blockedToday.toLocaleString()}
            </div>
          </div>
          <div>
            <div className="text-[11px] tracking-wide text-shell-muted">
              {t("adblock.stats.sevenDays")}
            </div>
            <div className="mt-1 tabular-nums text-[22px] tracking-[-0.03em] text-shell-text">
              {statistics.blockedLastSevenDays.toLocaleString()}
            </div>
          </div>
        </div>

        <section className="mb-5">
          <div className="mb-2 flex items-center justify-between gap-3">
            <h2 className="text-[13px] text-shell-text">{t("adblock.stats.byType")}</h2>
            {!statistics.recording ? (
              <span className="text-[11px] text-shell-danger">
                {t("adblock.stats.recordingUnavailable")}
              </span>
            ) : null}
          </div>
          {statistics.byResource.length === 0 ? (
            <div className="py-3 text-[12px] text-shell-muted">
              {t("adblock.stats.noData")}
            </div>
          ) : (
            <div className="space-y-2.5">
              {statistics.byResource.map((resource) => (
                <div key={resource.resourceType}>
                  <div className="mb-1 flex items-center justify-between gap-4 text-[12px]">
                    <span className="text-shell-muted">
                      {resourceTypeLabel(resource.resourceType)}
                    </span>
                    <span className="tabular-nums text-shell-text">
                      {resource.total.toLocaleString()}
                    </span>
                  </div>
                  <div className="h-1.5 overflow-hidden rounded-full bg-shell-track/35">
                    <div
                      className="h-full rounded-full bg-shell-accent/85"
                      style={{
                        width: `${Math.max(2, (resource.total / largestResourceCount) * 100)}%`,
                      }}
                    />
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="panel mb-5">
          <div className="flex items-center gap-4 px-5 py-3.5">
            <div className="min-w-0 flex-1">
              <div className="text-[13px] tracking-[-0.01em] text-shell-text">
                {t("adblock.enable.title")}
              </div>
              <div className="mt-0.5 text-[12px] text-shell-muted">
                {t("adblock.enable.description")}
              </div>
            </div>
            <Toggle
              on={enabled}
              label={t("adblock.enable.title")}
              onChange={setEnabled}
            />
          </div>
          <div className="flex items-center gap-4 border-t border-shell-border/50 px-5 py-3.5">
            <div className="min-w-0 flex-1">
              <div className="text-[13px] tracking-[-0.01em] text-shell-text">
                {t("adblock.cosmetic.title")}
              </div>
              <div className="mt-0.5 text-[12px] text-shell-muted">
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

        <div className="mb-2.5 flex items-center justify-between">
          <h2 className="text-[13px] font-medium tracking-[-0.01em] text-shell-text">
            {t("adblock.listsTitle")}
          </h2>
          <button
            type="button"
            className="inline-flex items-center gap-1.5 rounded-xl px-2.5 py-1.5 text-[12px] text-shell-muted hover:bg-shell-hover hover:text-shell-text"
          >
            <IconRefresh size={13} stroke={1.5} />
            {t("adblock.updateLists")}
          </button>
        </div>

        <section className="panel mb-5">
          <ul className="divide-y divide-shell-border/50">
            {lists.map((list) => (
              <li key={list.id} className="flex items-start gap-4 px-5 py-3.5">
                <div className="min-w-0 flex-1">
                  <div className="text-[13px] tracking-[-0.01em] text-shell-text">
                    {t(list.nameKey)}
                  </div>
                  <div className="mt-0.5 text-[12px] text-shell-muted">
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

        <h2 className="mb-2.5 text-[13px] font-medium tracking-[-0.01em] text-shell-text">
          {t("adblock.customTitle")}
        </h2>
        <section className="panel">
          <div className="flex gap-2 border-b border-shell-border/50 px-3.5 py-3">
            <input
              value={customRule}
              onChange={(e) => setCustomRule(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") addRule();
              }}
              placeholder={t("adblock.customPlaceholder")}
              className="h-9 min-w-0 flex-1 rounded-xl border border-shell-border/80 bg-shell px-3 font-mono text-[12px] text-shell-text outline-none placeholder:text-shell-muted focus:border-shell-accent/70"
            />
            <button
              type="button"
              onClick={addRule}
              className="inline-flex items-center gap-1.5 rounded-xl px-3 text-[12px] text-shell-text hover:bg-shell-hover"
            >
              <IconPlus size={14} stroke={1.5} />
              {t("adblock.addRule")}
            </button>
          </div>
          <ul className="divide-y divide-shell-border/50">
            {customRules.map((rule) => (
              <li
                key={rule}
                className="flex items-center gap-3 px-5 py-3 font-mono text-[12px] text-shell-text"
              >
                <span className="min-w-0 flex-1 truncate">{rule}</span>
                <button
                  type="button"
                  className="text-[11px] text-shell-muted hover:text-shell-danger"
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
