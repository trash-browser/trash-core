// Version metadata surfaced on the About settings page. Values are injected at
// shell build time by build.py (VITE_* env vars parsed from CEF's
// include/cef_version.h and the crate's Cargo.toml). When the shell is built
// standalone (plain `bun run build`) the fallbacks below are used instead.

const env = import.meta.env;

export const VERSIONS = {
  app: env.VITE_APP_VERSION?.trim() || "dev",
  cef: env.VITE_CEF_VERSION?.trim() || "unknown",
  chromium: env.VITE_CHROMIUM_VERSION?.trim() || "unknown",
} as const;
