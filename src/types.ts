export interface ClaudeUsageData {
  sessionPercentUsed: number;
  weeklyPercentUsed: number;
  sessionReset: string | null;
  weeklyReset: string | null;
  planType: string | null;
  extraUsageSpend: number | null;
  extraUsageLimit: number | null;
  lastUpdated: number | null;
}

export type DisplayMode = "usage" | "remaining";

export interface Settings {
  displayMode: DisplayMode;
  pollIntervalSeconds: number;
}

export interface UpdateInfo {
  version: string;
  body: string;
}
