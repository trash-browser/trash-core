import { useEffect, useState } from "react";
import { Link, Navigate, useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { IconArrowLeft } from "@tabler/icons-react";
import { LanguageSwitcher } from "../components/LanguageSwitcher";
import { ShellIcon } from "../components/ShellIcon";
import { Toggle } from "../components/Toggle";
import { isNavId } from "../data/settings";
import { getSubpage, isSubId } from "../data/subpages";
import { getSettings, setSetting } from "../native/ipc";

export function SettingsSubpage() {
  const { page, sub } = useParams();
  const { t } = useTranslation();
  const [toggles, setToggles] = useState<Record<string, boolean>>({});

  useEffect(() => {
    if (!isNavId(page) || !isSubId(page, sub)) return;
    let cancelled = false;
    void getSettings().then((values) => {
      if (cancelled) return;
      const def = getSubpage(page, sub!);
      if (!def) return;
      const loaded: Record<string, boolean> = {};
      for (const option of def.options) {
        const key =
          option.id === "toggle"
            ? `settings.${page}.${sub}`
            : `settings.${page}.${sub}.${option.id}`;
        if (typeof values[key] === "boolean") loaded[option.id] = values[key];
      }
      setToggles(loaded);
    });
    return () => {
      cancelled = true;
    };
  }, [page, sub]);

  if (!isNavId(page) || !isSubId(page, sub)) {
    return (
      <Navigate
        to={isNavId(page) ? `/settings/${page}` : `/settings/appearance`}
        replace
      />
    );
  }

  const def = getSubpage(page, sub!);
  if (!def) {
    return <Navigate to={`/settings/${page}`} replace />;
  }

  const title = t(def.row.titleKey);
  const description = def.row.descriptionKey
    ? t(def.row.descriptionKey)
    : undefined;
  const body = t(def.bodyKey, {
    title,
    defaultValue: t("sub.common.body", { title }),
  });

  return (
    <div className="mx-auto w-full max-w-[680px] px-6 py-8">
      <Link
        to={`/settings/${page}`}
        className="mb-5 inline-flex items-center gap-2 text-[13px] text-shell-accent hover:underline"
      >
        <IconArrowLeft size={16} stroke={1.6} />
        {t("sub.common.back")}
      </Link>

      <header className="mb-6">
        <h1 className="text-[28px] font-normal tracking-tight text-shell-text">
          {title}
        </h1>
        {description ? (
          <p className="mt-2 max-w-[540px] text-sm leading-5 text-shell-muted">
            {description}
          </p>
        ) : null}
      </header>

      <p className="mb-4 text-sm leading-5 text-shell-muted">{body}</p>

      <div className="flex flex-col gap-4">
        {def.localePicker ? <LanguageSwitcher /> : null}

        {def.options.length > 0 ? (
          <section className="overflow-hidden rounded-xl border border-shell-border/80 bg-shell-elevated">
            <div className="divide-y divide-shell-border/70">
              {def.options.map((option) => {
                const optTitle = t(option.titleKey, {
                  defaultValue: t("sub.common.option", { id: option.id }),
                });
                const optDesc = option.descriptionKey
                  ? t(option.descriptionKey, { defaultValue: "" })
                  : undefined;
                const toggleOn =
                  option.control.kind === "toggle"
                    ? (toggles[option.id] ?? option.control.on)
                    : false;

                return (
                  <div
                    key={option.id}
                    className="flex items-center gap-4 px-5 py-3.5 hover:bg-shell-hover/60"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-[13px] text-shell-text">{optTitle}</div>
                      {optDesc ? (
                        <div className="mt-0.5 text-xs text-shell-muted">
                          {optDesc}
                        </div>
                      ) : null}
                    </div>
                    {option.control.kind === "toggle" ? (
                      <Toggle
                        on={toggleOn}
                        label={optTitle}
                        onChange={(next) => {
                          const key =
                            option.id === "toggle"
                              ? `settings.${page}.${sub}`
                              : `settings.${page}.${sub}.${option.id}`;
                          setToggles((prev) => ({ ...prev, [option.id]: next }));
                          void setSetting(key, next).then((saved) => {
                            if (!saved) {
                              setToggles((prev) => ({
                                ...prev,
                                [option.id]: !next,
                              }));
                            }
                          });
                        }}
                      />
                    ) : (
                      <ShellIcon
                        name="chevron"
                        size={16}
                        className="text-shell-muted"
                      />
                    )}
                  </div>
                );
              })}
            </div>
          </section>
        ) : null}
      </div>
    </div>
  );
}
