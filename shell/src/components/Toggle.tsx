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
        "relative h-5 w-9 shrink-0 rounded-full transition-colors",
        on ? "bg-shell-accent" : "bg-shell-track",
      ].join(" ")}
    >
      <span
        className={[
          "absolute top-0.5 left-0.5 size-4 rounded-full bg-white shadow transition-transform",
          on ? "translate-x-4" : "translate-x-0",
        ].join(" ")}
      />
    </button>
  );
}
