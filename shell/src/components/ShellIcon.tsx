import {
  IconAccessible,
  IconChevronRight,
  IconDownload,
  IconInfoCircle,
  IconKey,
  IconLanguage,
  IconPalette,
  IconPower,
  IconRefresh,
  IconSearch,
  IconSettings,
  IconShield,
  IconUser,
  IconWorld,
  type Icon,
} from "@tabler/icons-react";

const ICONS: Record<string, Icon> = {
  user: IconUser,
  key: IconKey,
  shield: IconShield,
  palette: IconPalette,
  search: IconSearch,
  world: IconWorld,
  power: IconPower,
  language: IconLanguage,
  download: IconDownload,
  accessible: IconAccessible,
  settings: IconSettings,
  refresh: IconRefresh,
  info: IconInfoCircle,
  chevron: IconChevronRight,
};

export function ShellIcon({
  name,
  size = 20,
  className,
}: {
  name: string;
  size?: number;
  className?: string;
}) {
  const Cmp = ICONS[name];
  if (!Cmp) return null;
  return <Cmp size={size} stroke={1.6} className={className} />;
}
