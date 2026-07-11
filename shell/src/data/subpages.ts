import {
  PAGES,
  type NavId,
  type SettingControl,
  type SettingRow,
} from "./settings";

export type SubOption = {
  id: string;
  titleKey: string;
  descriptionKey?: string;
  control: SettingControl;
};

export type SubpageDef = {
  page: NavId;
  row: SettingRow;
  bodyKey: string;
  options: SubOption[];
  /** Embed locale picker (Languages → Preferred). */
  localePicker?: boolean;
};

function opt(
  page: NavId,
  rowId: string,
  id: string,
  control: SettingControl,
  withDesc = false,
): SubOption {
  const base = `sub.${page}.${rowId}.options.${id}`;
  return {
    id,
    titleKey: `${base}.title`,
    descriptionKey: withDesc ? `${base}.description` : undefined,
    control,
  };
}

/** Build subpage metadata for any settings row. */
export function getSubpage(page: NavId, subId: string): SubpageDef | null {
  const sections = PAGES[page];
  if (!sections) return null;
  let row: SettingRow | undefined;
  for (const section of sections) {
    row = section.rows.find((r) => r.id === subId);
    if (row) break;
  }
  if (!row) return null;

  const bodyKey = `sub.${page}.${subId}.body`;
  const options = SUB_OPTIONS[page]?.[subId] ?? defaultOptions(page, subId, row);

  return {
    page,
    row,
    bodyKey,
    options,
    localePicker: page === "languages" && subId === "preferred",
  };
}

function defaultOptions(page: NavId, rowId: string, row: SettingRow): SubOption[] {
  if (row.control.kind === "toggle") {
    return [
      opt(page, rowId, "toggle", { kind: "toggle", on: row.control.on }, true),
    ];
  }
  if (row.control.kind === "select") {
    return [
      opt(page, rowId, "a", { kind: "link" }, true),
      opt(page, rowId, "b", { kind: "link" }, true),
      opt(page, rowId, "c", { kind: "link" }, true),
    ];
  }
  return [
    opt(page, rowId, "a", { kind: "link" }, true),
    opt(page, rowId, "b", { kind: "toggle", on: true }, true),
    opt(page, rowId, "c", { kind: "link" }, true),
  ];
}

/** Optional curated option sets (falls back to defaults). */
const SUB_OPTIONS: Partial<Record<NavId, Record<string, SubOption[]>>> = {
  appearance: {
    theme: [
      opt("appearance", "theme", "device", { kind: "link" }, true),
      opt("appearance", "theme", "light", { kind: "link" }, true),
      opt("appearance", "theme", "dark", { kind: "link" }, true),
    ],
    "font-size": [
      opt("appearance", "font-size", "small", { kind: "link" }),
      opt("appearance", "font-size", "medium", { kind: "link" }),
      opt("appearance", "font-size", "large", { kind: "link" }),
      opt("appearance", "font-size", "xlarge", { kind: "link" }),
    ],
  },
  privacy: {
    "safe-browsing": [
      opt("privacy", "safe-browsing", "enhanced", { kind: "link" }, true),
      opt("privacy", "safe-browsing", "standard", { kind: "link" }, true),
      opt("privacy", "safe-browsing", "off", { kind: "link" }, true),
    ],
    clear: [
      opt("privacy", "clear", "lastHour", { kind: "link" }),
      opt("privacy", "clear", "lastDay", { kind: "link" }),
      opt("privacy", "clear", "allTime", { kind: "link" }),
    ],
  },
  search: {
    engine: [
      opt("search", "engine", "bing", { kind: "link" }),
      opt("search", "engine", "google", { kind: "link" }),
      opt("search", "engine", "duckduckgo", { kind: "link" }),
    ],
  },
  languages: {
    preferred: [], // locale picker UI
  },
};

export function listRowIds(page: NavId): string[] {
  return PAGES[page].flatMap((s) => s.rows.map((r) => r.id));
}

export function isSubId(page: NavId, sub: string | undefined): boolean {
  return !!sub && listRowIds(page).includes(sub);
}
