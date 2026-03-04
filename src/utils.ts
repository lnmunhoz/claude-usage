import type { DisplayMode } from "./types";

export const isTauri = () =>
  typeof window !== "undefined" &&
  !!((window as any).__TAURI_INTERNALS__ || (window as any).__TAURI__);

const THRESHOLD_LOW = 50;
const THRESHOLD_MED = 75;
const THRESHOLD_HIGH = 90;

export function getSessionColor(p: number) {
  if (p < THRESHOLD_LOW) return "#facc15";
  if (p < THRESHOLD_MED) return "#eab308";
  if (p < THRESHOLD_HIGH) return "#f97316";
  return "#ef4444";
}

export function getSessionGlow(p: number) {
  if (p < THRESHOLD_LOW) return "rgba(250, 204, 21, 0.5)";
  if (p < THRESHOLD_MED) return "rgba(234, 179, 8, 0.5)";
  if (p < THRESHOLD_HIGH) return "rgba(249, 115, 22, 0.5)";
  return "rgba(239, 68, 68, 0.5)";
}

export function getWeeklyColor(p: number) {
  if (p < THRESHOLD_LOW) return "#f97316";
  if (p < THRESHOLD_MED) return "#ea580c";
  if (p < THRESHOLD_HIGH) return "#ef4444";
  return "#dc2626";
}

export function getWeeklyGlow(p: number) {
  if (p < THRESHOLD_LOW) return "rgba(249, 115, 22, 0.5)";
  if (p < THRESHOLD_MED) return "rgba(234, 88, 12, 0.5)";
  if (p < THRESHOLD_HIGH) return "rgba(239, 68, 68, 0.5)";
  return "rgba(220, 38, 38, 0.5)";
}

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
