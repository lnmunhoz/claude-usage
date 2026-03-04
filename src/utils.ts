import type { DisplayMode } from "./types";

export const isTauri = () =>
  typeof window !== "undefined" &&
  !!((window as any).__TAURI_INTERNALS__ || (window as any).__TAURI__);

const THRESHOLD_LOW = 50;
const THRESHOLD_MED = 75;
const THRESHOLD_HIGH = 90;

function pick<T>(p: number, low: T, med: T, high: T, max: T): T {
  if (p < THRESHOLD_LOW) return low;
  if (p < THRESHOLD_MED) return med;
  if (p < THRESHOLD_HIGH) return high;
  return max;
}

export const getSessionColor = (p: number) =>
  pick(p, "#facc15", "#eab308", "#f97316", "#ef4444");

export const getSessionGlow = (p: number) =>
  pick(p, "rgba(250, 204, 21, 0.5)", "rgba(234, 179, 8, 0.5)", "rgba(249, 115, 22, 0.5)", "rgba(239, 68, 68, 0.5)");

export const getWeeklyColor = (p: number) =>
  pick(p, "#f97316", "#ea580c", "#ef4444", "#dc2626");

export const getWeeklyGlow = (p: number) =>
  pick(p, "rgba(249, 115, 22, 0.5)", "rgba(234, 88, 12, 0.5)", "rgba(239, 68, 68, 0.5)", "rgba(220, 38, 38, 0.5)");

export function clampFill(p: number) {
  return Math.min(100, Math.max(0, p));
}

export function computeFill(percent: number, mode: DisplayMode): number {
  return mode === "remaining" ? clampFill(100 - percent) : clampFill(percent);
}

export function formatResetTime(isoString: string | null): string | null {
  if (!isoString) return null;
  try {
    const resetDate = new Date(isoString);
    const now = new Date();
    const diffMs = resetDate.getTime() - now.getTime();
    if (diffMs <= 0) return "now";
    const hours = Math.floor(diffMs / (1000 * 60 * 60));
    const minutes = Math.floor((diffMs % (1000 * 60 * 60)) / (1000 * 60));
    if (hours > 0) return `${hours}h ${minutes}m`;
    return `${minutes}m`;
  } catch {
    return null;
  }
}

export function formatWeeklyResetTime(isoString: string | null): string | null {
  if (!isoString) return null;
  try {
    const resetDate = new Date(isoString);
    const day = resetDate.toLocaleDateString(undefined, { weekday: "short" });
    const time = resetDate.toLocaleTimeString(undefined, {
      hour: "numeric",
      minute: "2-digit",
      hour12: true,
    });
    return `${day} ${time}`;
  } catch {
    return null;
  }
}
