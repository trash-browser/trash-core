import { useId } from "react";
import { useTranslation } from "react-i18next";
import { IconSearch, IconX } from "@tabler/icons-react";

type Props = {
  value: string;
  onChange: (value: string) => void;
};

export function SettingsSearch({ value, onChange }: Props) {
  const id = useId();
  const { t } = useTranslation();

  return (
    <div className="border-b border-shell-border/50 bg-shell/80 px-7 py-3 backdrop-blur-sm">
      <label
        htmlFor={id}
        className="mx-auto flex h-10 max-w-[640px] items-center gap-2.5 rounded-xl border border-shell-border/80 bg-shell-elevated px-3.5 transition-[border-color] focus-within:border-shell-accent/60"
      >
        <IconSearch size={16} stroke={1.5} className="text-shell-muted" />
        <input
          id={id}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={t("shell.searchPlaceholder")}
          className="h-full w-full bg-transparent text-[13px] tracking-[-0.01em] text-shell-text outline-none placeholder:text-shell-muted"
        />
        {value ? (
          <button
            type="button"
            aria-label={t("shell.clearSearch")}
            onClick={() => onChange("")}
            className="rounded-lg p-1 text-shell-muted hover:bg-shell-hover hover:text-shell-text"
          >
            <IconX size={14} stroke={1.5} />
          </button>
        ) : null}
      </label>
    </div>
  );
}
