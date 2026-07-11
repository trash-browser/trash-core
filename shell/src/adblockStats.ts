import { useSyncExternalStore } from "react";

export type AdblockResourceCount = {
  resourceType: string;
  total: number;
};

export type AdblockStatistics = {
  totalBlocked: number;
  blockedToday: number;
  blockedLastSevenDays: number;
  byResource: AdblockResourceCount[];
  recording: boolean;
};

export type AdblockFilterStatus = {
  state: "loading" | "active" | "error";
  listsLoaded: number;
  lastError: string | null;
};

const EMPTY_STATISTICS: AdblockStatistics = {
  totalBlocked: 0,
  blockedToday: 0,
  blockedLastSevenDays: 0,
  byResource: [],
  recording: false,
};

const LOADING_FILTER_STATUS: AdblockFilterStatus = {
  state: "loading",
  listsLoaded: 0,
  lastError: null,
};

declare global {
  interface Window {
    __trashAdblockStats?: AdblockStatistics;
    __trashAdblockStatus?: AdblockFilterStatus;
  }
}

function isFilterStatus(value: unknown): value is AdblockFilterStatus {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<AdblockFilterStatus>;
  return (
    (candidate.state === "loading" ||
      candidate.state === "active" ||
      candidate.state === "error") &&
    typeof candidate.listsLoaded === "number" &&
    (candidate.lastError === null || typeof candidate.lastError === "string")
  );
}

function isStatistics(value: unknown): value is AdblockStatistics {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<AdblockStatistics>;
  return (
    typeof candidate.totalBlocked === "number" &&
    typeof candidate.blockedToday === "number" &&
    typeof candidate.blockedLastSevenDays === "number" &&
    Array.isArray(candidate.byResource) &&
    typeof candidate.recording === "boolean"
  );
}

function initialStatistics(): AdblockStatistics {
  return isStatistics(window.__trashAdblockStats)
    ? window.__trashAdblockStats
    : EMPTY_STATISTICS;
}

function subscribeStatistics(onStoreChange: () => void) {
  const receive = () => onStoreChange();
  window.addEventListener("trash-adblock-stats", receive);
  return () => window.removeEventListener("trash-adblock-stats", receive);
}

function initialFilterStatus(): AdblockFilterStatus {
  return isFilterStatus(window.__trashAdblockStatus)
    ? window.__trashAdblockStatus
    : LOADING_FILTER_STATUS;
}

function subscribeFilterStatus(onStoreChange: () => void) {
  const receive = () => onStoreChange();
  window.addEventListener("trash-adblock-status", receive);
  return () => window.removeEventListener("trash-adblock-status", receive);
}

export function useAdblockStatistics() {
  return useSyncExternalStore(
    subscribeStatistics,
    initialStatistics,
    () => EMPTY_STATISTICS,
  );
}

export function useAdblockFilterStatus() {
  return useSyncExternalStore(
    subscribeFilterStatus,
    initialFilterStatus,
    () => LOADING_FILTER_STATUS,
  );
}
