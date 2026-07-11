export type Shortcut = {
  id: string;
  labelKey: string;
  url: string;
  letter: string;
  tint: string;
};

export const NTP_SHORTCUTS: Shortcut[] = [
  { id: "apple", labelKey: "ntp.shortcuts.apple", url: "https://www.apple.com", letter: "A", tint: "#2a2a2e" },
  { id: "wiki", labelKey: "ntp.shortcuts.wiki", url: "https://www.wikipedia.org", letter: "W", tint: "#2c3036" },
  { id: "github", labelKey: "ntp.shortcuts.github", url: "https://github.com", letter: "G", tint: "#252a30" },
  { id: "bing", labelKey: "ntp.shortcuts.bing", url: "https://www.bing.com", letter: "B", tint: "#243038" },
  { id: "youtube", labelKey: "ntp.shortcuts.youtube", url: "https://www.youtube.com", letter: "Y", tint: "#342626" },
  { id: "mail", labelKey: "ntp.shortcuts.mail", url: "https://mail.google.com", letter: "M", tint: "#2c2832" },
  { id: "maps", labelKey: "ntp.shortcuts.maps", url: "https://maps.google.com", letter: "P", tint: "#24322c" },
  { id: "news", labelKey: "ntp.shortcuts.news", url: "https://news.google.com", letter: "N", tint: "#322c24" },
];

export type DownloadItem = {
  id: string;
  name: string;
  source: string;
  size: string;
  status: "complete" | "progress" | "paused" | "failed";
  progress?: number;
  path: string;
  timeKey: string;
};

export const MOCK_DOWNLOADS: DownloadItem[] = [
  {
    id: "1",
    name: "TrashBrowser-Setup-0.1.0.exe",
    source: "cdn.trashbrowser.app",
    size: "86.4 MB",
    status: "complete",
    path: "C:\\Users\\Downloads",
    timeKey: "downloadsMgr.time.today",
  },
  {
    id: "2",
    name: "design-system.fig",
    source: "www.figma.com",
    size: "24.1 MB",
    status: "progress",
    progress: 62,
    path: "C:\\Users\\Downloads",
    timeKey: "downloadsMgr.time.now",
  },
  {
    id: "3",
    name: "report-q2.pdf",
    source: "drive.google.com",
    size: "3.2 MB",
    status: "paused",
    progress: 41,
    path: "C:\\Users\\Downloads",
    timeKey: "downloadsMgr.time.yesterday",
  },
  {
    id: "4",
    name: "sample-pack.zip",
    source: "files.example.com",
    size: "512 MB",
    status: "failed",
    path: "C:\\Users\\Downloads",
    timeKey: "downloadsMgr.time.week",
  },
  {
    id: "5",
    name: "wallpaper-4k.jpg",
    source: "unsplash.com",
    size: "8.7 MB",
    status: "complete",
    path: "C:\\Users\\Downloads",
    timeKey: "downloadsMgr.time.week",
  },
];

export type FilterList = {
  id: string;
  nameKey: string;
  descKey: string;
  rules: string;
  updatedKey: string;
  enabled: boolean;
};

export const MOCK_FILTER_LISTS: FilterList[] = [
  {
    id: "easylist",
    nameKey: "adblock.lists.easylist.name",
    descKey: "adblock.lists.easylist.desc",
    rules: "78,412",
    updatedKey: "adblock.lists.updated.hours",
    enabled: true,
  },
  {
    id: "easyprivacy",
    nameKey: "adblock.lists.easyprivacy.name",
    descKey: "adblock.lists.easyprivacy.desc",
    rules: "42,108",
    updatedKey: "adblock.lists.updated.today",
    enabled: true,
  },
  {
    id: "fanboy",
    nameKey: "adblock.lists.fanboy.name",
    descKey: "adblock.lists.fanboy.desc",
    rules: "19,330",
    updatedKey: "adblock.lists.updated.days",
    enabled: false,
  },
  {
    id: "trash",
    nameKey: "adblock.lists.trash.name",
    descKey: "adblock.lists.trash.desc",
    rules: "1,204",
    updatedKey: "adblock.lists.updated.hours",
    enabled: true,
  },
];
