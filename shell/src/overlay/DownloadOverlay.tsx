import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { IconDownload } from "@tabler/icons-react";
import { OverlayActions, OverlayButton, OverlayShell } from "./OverlayShell";
import { postOverlayAction } from "./bridge";

export function DownloadOverlay() {
  const { t } = useTranslation();
  const [params] = useSearchParams();
  const [saving, setSaving] = useState(true);

  const file = useMemo(
    () => ({
      name: params.get("name") ?? "document.pdf",
      size: params.get("size") ?? "2.4 MB",
      from: params.get("from") ?? "example.com",
      url: params.get("url") ?? "https://example.com/document.pdf",
    }),
    [params],
  );

  return (
    <OverlayShell
      align="bottom-right"
      dim={false}
      onBackdrop={() => postOverlayAction("download", "close")}
    >
      <div className="flex gap-3 px-4 pt-4 pb-3">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-shell-accent-dim text-shell-accent">
          <IconDownload size={20} stroke={1.6} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-[13px] font-medium text-shell-text">
            {t("overlay.download.title")}
          </div>
          <div className="mt-0.5 truncate text-xs text-shell-muted">
            {file.name}
          </div>
          <div className="mt-1 text-[11px] text-shell-muted">
            {file.size} · {file.from}
          </div>
        </div>
      </div>

      <label className="mx-4 mb-3 flex items-center gap-2 text-[12px] text-shell-muted">
        <input
          type="checkbox"
          checked={saving}
          onChange={(e) => setSaving(e.target.checked)}
          className="accent-shell-accent"
        />
        {t("overlay.download.askPath")}
      </label>

      <OverlayActions>
        <OverlayButton onClick={() => postOverlayAction("download", "cancel")}>
          {t("overlay.common.cancel")}
        </OverlayButton>
        <OverlayButton
          variant="primary"
          onClick={() =>
            postOverlayAction("download", "save", {
              ...file,
              askPath: saving,
            })
          }
        >
          {t("overlay.download.save")}
        </OverlayButton>
      </OverlayActions>
    </OverlayShell>
  );
}
