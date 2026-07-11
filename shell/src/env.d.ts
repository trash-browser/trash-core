// Build-time environment variables injected by build.py when it runs
// `bun run build` for the shell. See build.py -> build_shell().
interface ImportMetaEnv {
  /** Trash Browser application version (Cargo package version). */
  readonly VITE_APP_VERSION?: string;
  /** Embedded CEF distribution version, e.g. "150.0.11". */
  readonly VITE_CEF_VERSION?: string;
  /** Underlying Chromium version, e.g. "150.0.7871.115". */
  readonly VITE_CHROMIUM_VERSION?: string;
}

declare module "*.png" {
  const src: string;
  export default src;
}

