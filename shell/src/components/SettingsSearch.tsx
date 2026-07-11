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
    <div className="border-b border-shell-border/80 bg-shell px-6 py-3">
      <label
        htmlFor={id}
        className="mx-auto flex h-10 max-w-[680px] items-center gap-3 rounded-full border border-shell-border bg-shell-elevated px-4 focus-within:border-shell-accent"
      >
        <IconSearch size={18} stroke={1.6} className="text-shell-muted" />
        <input
          id={id}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={t("shell.searchPlaceholder")}
          className="h-full w-full bg-transparent text-[13px] text-shell-text outline-none placeholder:text-shell-muted"
        />
        {value ? (
          <button
            type="button"
            aria-label={t("shell.clearSearch")}
            onClick={() => onChange("")}
            className="rounded-full p-0.5 text-shell-muted hover:bg-shell-hover hover:text-shell-text"
          >
            <IconX size={16} stroke={1.6} />
          </button>
        ) : null}
      </label>
    </div>
  );
}
