export type NavId =
  | "you"
  | "autofill"
  | "privacy"
  | "appearance"
  | "search"
  | "default"
  | "startup"
  | "languages"
  | "downloads"
  | "a11y"
  | "system"
  | "reset"
  | "about";

export const NAV_IDS: NavId[] = [
  "you",
  "autofill",
  "privacy",
  "appearance",
  "search",
  "default",
  "startup",
  "languages",
  "downloads",
  "a11y",
  "system",
  "reset",
  "about",
];

export const DEFAULT_NAV: NavId = "appearance";

export function isNavId(value: string | undefined): value is NavId {
  return !!value && (NAV_IDS as string[]).includes(value);
}

export type SettingControl =
  | { kind: "link"; valueKey?: string }
  | { kind: "toggle"; on: boolean }
  | { kind: "select"; valueKey: string };

export type SettingRow = {
  id: string;
  titleKey: string;
  descriptionKey?: string;
  control: SettingControl;
};

export type SettingSection = {
  id: string;
  titleKey: string;
  descriptionKey?: string;
  rows: SettingRow[];
};

export type NavItem = {
  id: NavId;
  labelKey: string;
  icon: string;
};

export const NAV: NavItem[] = [
  { id: "you", labelKey: "nav.you", icon: "user" },
  { id: "autofill", labelKey: "nav.autofill", icon: "key" },
  { id: "privacy", labelKey: "nav.privacy", icon: "shield" },
  { id: "appearance", labelKey: "nav.appearance", icon: "palette" },
  { id: "search", labelKey: "nav.search", icon: "search" },
  { id: "default", labelKey: "nav.default", icon: "world" },
  { id: "startup", labelKey: "nav.startup", icon: "power" },
  { id: "languages", labelKey: "nav.languages", icon: "language" },
  { id: "downloads", labelKey: "nav.downloads", icon: "download" },
  { id: "a11y", labelKey: "nav.a11y", icon: "accessible" },
  { id: "system", labelKey: "nav.system", icon: "settings" },
  { id: "reset", labelKey: "nav.reset", icon: "refresh" },
  { id: "about", labelKey: "nav.about", icon: "info" },
];

export const PAGES: Record<NavId, SettingSection[]> = {
  you: [
    {
      id: "profile",
      titleKey: "pages.you.title",
      descriptionKey: "pages.you.description",
      rows: [
        {
          id: "sync",
          titleKey: "pages.you.sync.title",
          descriptionKey: "pages.you.sync.description",
          control: { kind: "link" },
        },
        {
          id: "services",
          titleKey: "pages.you.services.title",
          descriptionKey: "pages.you.services.description",
          control: { kind: "link" },
        },
      ],
    },
  ],
  autofill: [
    {
      id: "passwords",
      titleKey: "pages.autofill.title",
      rows: [
        {
          id: "password-manager",
          titleKey: "pages.autofill.passwordManager.title",
          descriptionKey: "pages.autofill.passwordManager.description",
          control: { kind: "link" },
        },
        {
          id: "payment",
          titleKey: "pages.autofill.payment.title",
          descriptionKey: "pages.autofill.payment.description",
          control: { kind: "link" },
        },
        {
          id: "addresses",
          titleKey: "pages.autofill.addresses.title",
          descriptionKey: "pages.autofill.addresses.description",
          control: { kind: "link" },
        },
      ],
    },
  ],
  privacy: [
    {
      id: "privacy",
      titleKey: "pages.privacy.title",
      rows: [
        {
          id: "clear",
          titleKey: "pages.privacy.clear.title",
          descriptionKey: "pages.privacy.clear.description",
          control: { kind: "link" },
        },
        {
          id: "cookies",
          titleKey: "pages.privacy.cookies.title",
          descriptionKey: "pages.privacy.cookies.description",
          control: { kind: "toggle", on: true },
        },
        {
          id: "safe-browsing",
          titleKey: "pages.privacy.safeBrowsing.title",
          descriptionKey: "pages.privacy.safeBrowsing.description",
          control: { kind: "select", valueKey: "pages.privacy.safeBrowsing.value" },
        },
        {
          id: "site-settings",
          titleKey: "pages.privacy.siteSettings.title",
          descriptionKey: "pages.privacy.siteSettings.description",
          control: { kind: "link" },
        },
      ],
    },
  ],
  appearance: [
    {
      id: "appearance",
      titleKey: "pages.appearance.title",
      rows: [
        {
          id: "theme",
          titleKey: "pages.appearance.theme.title",
          descriptionKey: "pages.appearance.theme.description",
          control: { kind: "select", valueKey: "pages.appearance.theme.value" },
        },
        {
          id: "toolbar",
          titleKey: "pages.appearance.homeButton.title",
          control: { kind: "toggle", on: false },
        },
        {
          id: "bookmarks-bar",
          titleKey: "pages.appearance.bookmarksBar.title",
          control: { kind: "toggle", on: true },
        },
        {
          id: "font-size",
          titleKey: "pages.appearance.fontSize.title",
          control: { kind: "select", valueKey: "pages.appearance.fontSize.value" },
        },
        {
          id: "zoom",
          titleKey: "pages.appearance.zoom.title",
          control: { kind: "select", valueKey: "pages.appearance.zoom.value" },
        },
      ],
    },
  ],
  search: [
    {
      id: "search",
      titleKey: "pages.search.title",
      descriptionKey: "pages.search.description",
      rows: [
        {
          id: "engine",
          titleKey: "pages.search.engine.title",
          control: { kind: "select", valueKey: "pages.search.engine.value" },
        },
        {
          id: "manage",
          titleKey: "pages.search.manage.title",
          control: { kind: "link" },
        },
      ],
    },
  ],
  default: [
    {
      id: "default",
      titleKey: "pages.default.title",
      rows: [
        {
          id: "make-default",
          titleKey: "pages.default.makeDefault.title",
          descriptionKey: "pages.default.makeDefault.description",
          control: { kind: "link", valueKey: "pages.default.makeDefault.action" },
        },
      ],
    },
  ],
  startup: [
    {
      id: "startup",
      titleKey: "pages.startup.title",
      rows: [
        {
          id: "open",
          titleKey: "pages.startup.openNewTab.title",
          control: { kind: "toggle", on: true },
        },
        {
          id: "continue",
          titleKey: "pages.startup.continue.title",
          control: { kind: "toggle", on: false },
        },
        {
          id: "specific",
          titleKey: "pages.startup.specific.title",
          control: { kind: "link" },
        },
      ],
    },
  ],
  languages: [
    {
      id: "languages",
      titleKey: "pages.languages.title",
      rows: [
        {
          id: "preferred",
          titleKey: "pages.languages.preferred.title",
          descriptionKey: "pages.languages.preferred.description",
          control: { kind: "link" },
        },
        {
          id: "spellcheck",
          titleKey: "pages.languages.spellcheck.title",
          control: { kind: "toggle", on: true },
        },
        {
          id: "translate",
          titleKey: "pages.languages.translate.title",
          control: { kind: "toggle", on: true },
        },
      ],
    },
  ],
  downloads: [
    {
      id: "downloads",
      titleKey: "pages.downloads.title",
      rows: [
        {
          id: "location",
          titleKey: "pages.downloads.location.title",
          descriptionKey: "pages.downloads.location.description",
          control: { kind: "link", valueKey: "pages.downloads.location.action" },
        },
        {
          id: "ask",
          titleKey: "pages.downloads.ask.title",
          control: { kind: "toggle", on: false },
        },
      ],
    },
  ],
  a11y: [
    {
      id: "a11y",
      titleKey: "pages.a11y.title",
      rows: [
        {
          id: "captions",
          titleKey: "pages.a11y.captions.title",
          control: { kind: "link" },
        },
        {
          id: "highlight",
          titleKey: "pages.a11y.highlight.title",
          control: { kind: "toggle", on: false },
        },
        {
          id: "tab-hover",
          titleKey: "pages.a11y.tabHover.title",
          control: { kind: "toggle", on: true },
        },
      ],
    },
  ],
  system: [
    {
      id: "system",
      titleKey: "pages.system.title",
      rows: [
        {
          id: "hw",
          titleKey: "pages.system.hw.title",
          control: { kind: "toggle", on: true },
        },
        {
          id: "bg",
          titleKey: "pages.system.bg.title",
          control: { kind: "toggle", on: false },
        },
        {
          id: "proxy",
          titleKey: "pages.system.proxy.title",
          control: { kind: "link" },
        },
      ],
    },
  ],
  reset: [
    {
      id: "reset",
      titleKey: "pages.reset.title",
      descriptionKey: "pages.reset.description",
      rows: [
        {
          id: "restore",
          titleKey: "pages.reset.restore.title",
          descriptionKey: "pages.reset.restore.description",
          control: { kind: "link", valueKey: "pages.reset.restore.action" },
        },
      ],
    },
  ],
  about: [
    {
      id: "about",
      titleKey: "pages.about.title",
      rows: [
        {
          id: "version",
          titleKey: "pages.about.version.title",
          descriptionKey: "pages.about.version.description",
          control: { kind: "link", valueKey: "pages.about.version.action" },
        },
        {
          id: "help",
          titleKey: "pages.about.help.title",
          control: { kind: "link" },
        },
      ],
    },
  ],
};
