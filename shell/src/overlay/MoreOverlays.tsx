import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { IconAlertTriangle, IconInfoCircle } from "@tabler/icons-react";
import { OverlayActions, OverlayButton, OverlayShell } from "./OverlayShell";
import { postOverlayAction } from "./bridge";

export function ConfirmOverlay() {
  const { t } = useTranslation();
  const [params] = useSearchParams();
  const danger = params.get("danger") === "1";

  const data = useMemo(
    () => ({
      title: params.get("title") ?? t("overlay.confirm.title"),
      body: params.get("body") ?? t("overlay.confirm.body"),
    }),
    [params, t],
  );

  const Icon = danger ? IconAlertTriangle : IconInfoCircle;

  return (
    <OverlayShell onBackdrop={() => postOverlayAction("confirm", "cancel")}>
      <div className="flex gap-3 px-5 pt-5 pb-3">
        <div
          className={[
            "flex size-10 shrink-0 items-center justify-center rounded-xl",
            danger
              ? "bg-shell-danger/15 text-shell-danger"
              : "bg-shell-accent-dim text-shell-accent",
          ].join(" ")}
        >
          <Icon size={20} stroke={1.6} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-[15px] font-medium text-shell-text">
            {data.title}
          </div>
          <div className="mt-1 text-[13px] leading-5 text-shell-muted">
            {data.body}
          </div>
        </div>
      </div>
      <OverlayActions>
        <OverlayButton onClick={() => postOverlayAction("confirm", "cancel")}>
          {t("overlay.common.cancel")}
        </OverlayButton>
        <OverlayButton
          variant={danger ? "danger" : "primary"}
          onClick={() => postOverlayAction("confirm", "allow")}
        >
          {t("overlay.common.confirm")}
        </OverlayButton>
      </OverlayActions>
    </OverlayShell>
  );
}

export function AuthOverlay() {
  const { t } = useTranslation();
  const [params] = useSearchParams();
  const [user, setUser] = useState("");
  const [pass, setPass] = useState("");
  const host = params.get("host") ?? "example.com";

  return (
    <OverlayShell onBackdrop={() => postOverlayAction("auth", "cancel")}>
      <div className="px-5 pt-5 pb-2">
        <div className="text-[15px] font-medium text-shell-text">
          {t("overlay.auth.title")}
        </div>
        <div className="mt-1 text-[13px] text-shell-muted">
          {t("overlay.auth.body", { host })}
        </div>
        <div className="mt-4 flex flex-col gap-2">
          <input
            value={user}
            onChange={(e) => setUser(e.target.value)}
            placeholder={t("overlay.auth.username")}
            className="h-9 rounded-lg border border-shell-border bg-shell px-3 text-[13px] text-shell-text outline-none focus:border-shell-accent"
          />
          <input
            type="password"
            value={pass}
            onChange={(e) => setPass(e.target.value)}
            placeholder={t("overlay.auth.password")}
            className="h-9 rounded-lg border border-shell-border bg-shell px-3 text-[13px] text-shell-text outline-none focus:border-shell-accent"
          />
        </div>
      </div>
      <OverlayActions>
        <OverlayButton onClick={() => postOverlayAction("auth", "cancel")}>
          {t("overlay.common.cancel")}
        </OverlayButton>
        <OverlayButton
          variant="primary"
          onClick={() =>
            postOverlayAction("auth", "allow", {
              username: user,
              password: pass,
              host,
            })
          }
        >
          {t("overlay.auth.signIn")}
        </OverlayButton>
      </OverlayActions>
    </OverlayShell>
  );
}

export function FindOverlay() {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [match, setMatch] = useState(0);

  return (
    <OverlayShell
      align="top-right"
      dim={false}
      className="!w-[min(100%-24px,360px)]"
      onBackdrop={() => postOverlayAction("find", "close")}
    >
      <div className="flex items-center gap-2 px-3 py-2">
        <input
          autoFocus
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setMatch(e.target.value ? 1 : 0);
            postOverlayAction("find", "show", { query: e.target.value });
          }}
          placeholder={t("overlay.find.placeholder")}
          className="h-9 min-w-0 flex-1 rounded-lg border border-shell-border bg-shell px-3 text-[13px] text-shell-text outline-none focus:border-shell-accent"
        />
        <span className="shrink-0 text-[11px] text-shell-muted">
          {query ? `${match}/3` : ""}
        </span>
        <OverlayButton onClick={() => postOverlayAction("find", "close")}>
          {t("overlay.common.close")}
        </OverlayButton>
      </div>
    </OverlayShell>
  );
}
