import { type ReactNode } from "react";
import { motion } from "framer-motion";
import {
  overlayBackdropVariants,
  overlayCardVariants,
} from "../motion";

type Props = {
  children: ReactNode;
  /** Align popup within the transparent CEF window */
  align?: "center" | "top" | "bottom-right" | "top-right";
  /** Dim backdrop (still click-through transparent outside card for CEF) */
  dim?: boolean;
  onBackdrop?: () => void;
  className?: string;
  wide?: boolean;
};

const alignClass: Record<NonNullable<Props["align"]>, string> = {
  center: "items-center justify-center",
  top: "items-start justify-center pt-10",
  "bottom-right": "items-end justify-end p-4",
  "top-right": "items-start justify-end p-4",
};

export function OverlayShell({
  children,
  align = "center",
  dim = true,
  onBackdrop,
  className = "",
  wide = false,
}: Props) {
  return (
    <div
      className={`overlay-root flex h-full min-h-0 w-full ${alignClass[align]}`}
      data-trash-overlay="1"
    >
      <motion.div
        className={[
          "absolute inset-0",
          dim ? "bg-black/45" : "bg-transparent",
        ].join(" ")}
        variants={overlayBackdropVariants}
        initial="initial"
        animate="animate"
        exit="exit"
        onClick={onBackdrop}
      />
      <motion.div
        role="dialog"
        aria-modal="true"
        className={[
          "relative z-10 w-[min(100%-24px,420px)] overflow-hidden rounded-2xl border border-shell-border/90 bg-shell-elevated shadow-[0_16px_48px_rgba(0,0,0,0.45)]",
          wide ? "w-[min(100%-24px,520px)]" : "",
          className,
        ].join(" ")}
        variants={overlayCardVariants}
        initial="initial"
        animate="animate"
        exit="exit"
        onClick={(e) => e.stopPropagation()}
      >
        {children}
      </motion.div>
    </div>
  );
}

export function OverlayActions({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center justify-end gap-2 border-t border-shell-border/80 bg-shell/40 px-4 py-3">
      {children}
    </div>
  );
}

export function OverlayButton({
  children,
  variant = "ghost",
  onClick,
}: {
  children: ReactNode;
  variant?: "ghost" | "primary" | "danger";
  onClick?: () => void;
}) {
  const styles = {
    ghost:
      "text-shell-accent hover:bg-shell-accent-dim",
    primary:
      "bg-shell-accent text-[#202124] hover:brightness-110",
    danger:
      "text-shell-danger hover:bg-shell-danger/15",
  } as const;

  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full px-4 py-1.5 text-[13px] font-medium transition-colors ${styles[variant]}`}
    >
      {children}
    </button>
  );
}
