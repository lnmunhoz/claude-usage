import type { DisplayMode } from "./types";

export const isTauri = () =>
  typeof window !== "undefined" &&
  !!((window as any).__TAURI_INTERNALS__ || (window as any).__TAURI__);

export function getSessionColor(p: number) {
  if (p < 50) return "#facc15";
  if (p < 75) return "#eab308";
  if (p < 90) return "#f97316";
  return "#ef4444";
}

export function getSessionGlow(p: number) {
  if (p < 50) return "rgba(250, 204, 21, 0.5)";
  if (p < 75) return "rgba(234, 179, 8, 0.5)";
  if (p < 90) return "rgba(249, 115, 22, 0.5)";
  return "rgba(239, 68, 68, 0.5)";
}

export function getWeeklyColor(p: number) {
  if (p < 50) return "#f97316";
  if (p < 75) return "#ea580c";
  if (p < 90) return "#ef4444";
  return "#dc2626";
}

export function getWeeklyGlow(p: number) {
  if (p < 50) return "rgba(249, 115, 22, 0.5)";
  if (p < 75) return "rgba(234, 88, 12, 0.5)";
  if (p < 90) return "rgba(239, 68, 68, 0.5)";
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
