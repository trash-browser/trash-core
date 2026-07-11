import { useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  IconCurrentLocation,
  IconMicrophone,
  IconVideo,
  IconBell,
  IconLock,
} from "@tabler/icons-react";
import { OverlayActions, OverlayButton, OverlayShell } from "./OverlayShell";
import { postOverlayAction } from "./bridge";

const ICONS = {
  camera: IconVideo,
  microphone: IconMicrophone,
  location: IconCurrentLocation,
  notifications: IconBell,
  generic: IconLock,
} as const;

export function PermissionOverlay() {
  const { t } = useTranslation();
  const [params] = useSearchParams();

  const data = useMemo(() => {
    const raw = params.get("type") ?? "camera";
    const type = (raw in ICONS ? raw : "generic") as keyof typeof ICONS;
    return {
      type,
      origin: params.get("origin") ?? "https://example.com",
    };
  }, [params]);

  const Icon = ICONS[data.type];

  return (
    <OverlayShell
      align="top"
      onBackdrop={() => postOverlayAction("permission", "deny")}
    >
      <div className="flex gap-3 px-5 pt-5 pb-2">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-shell-accent-dim text-shell-accent">
          <Icon size={20} stroke={1.6} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-[15px] font-medium text-shell-text">
            {t(`overlay.permission.types.${data.type}.title`, {
              defaultValue: t("overlay.permission.title"),
            })}
          </div>
          <div className="mt-1 text-[13px] leading-5 text-shell-muted">
            {t("overlay.permission.body", { origin: data.origin })}
          </div>
        </div>
      </div>

      <OverlayActions>
        <OverlayButton
          onClick={() =>
            postOverlayAction("permission", "deny", {
              origin: data.origin,
              type: data.type,
            })
          }
        >
          {t("overlay.permission.block")}
        </OverlayButton>
        <OverlayButton
          onClick={() =>
            postOverlayAction("permission", "once", {
              origin: data.origin,
              type: data.type,
            })
          }
        >
          {t("overlay.permission.once")}
        </OverlayButton>
        <OverlayButton
          variant="primary"
          onClick={() =>
            postOverlayAction("permission", "allow", {
              origin: data.origin,
              type: data.type,
            })
          }
        >
          {t("overlay.permission.allow")}
        </OverlayButton>
      </OverlayActions>
    </OverlayShell>
  );
}
