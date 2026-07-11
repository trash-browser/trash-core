type Props = {
  on: boolean;
  onChange: (next: boolean) => void;
  label: string;
};

export function Toggle({ on, onChange, label }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => onChange(!on)}
      className={[
        "relative h-[22px] w-[38px] shrink-0 rounded-full transition-colors duration-200",
        on ? "bg-shell-accent" : "bg-shell-track",
      ].join(" ")}
    >
      <span
        className={[
          "absolute top-[3px] left-[3px] size-4 rounded-full bg-white shadow-sm transition-transform duration-200",
          on ? "translate-x-[16px]" : "translate-x-0",
        ].join(" ")}
      />
    </button>
  );
}
