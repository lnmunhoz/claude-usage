import { openUrl } from "@tauri-apps/plugin-opener";
import claudeMascot from "../assets/claude-mascot.png";
import type { ClaudeUsageData, DisplayMode } from "../types";
import {
  getSessionColor,
  getSessionGlow,
  getWeeklyColor,
  getWeeklyGlow,
  computeFill,
  formatResetTime,
} from "../utils";

interface UsageViewProps {
  data: ClaudeUsageData;
  displayMode: DisplayMode;
  onDisconnect: () => void;
}

interface UsageBarProps {
  label: string;
  fill: number;
  color: string;
  glow: string;
  displayMode: DisplayMode;
  reset: string | null;
}

function UsageBar({ label, fill, color, glow, displayMode, reset }: UsageBarProps) {
  return (
    <div className="panel-bar-group">
      <div className="panel-bar-label-row">
        <span className="panel-bar-name">{label}</span>
        <span className="panel-bar-pct">
          {fill.toFixed(1)}% {displayMode === "remaining" ? "left" : "used"}
        </span>
      </div>
      <div className="panel-bar-track">
        <div
          className="panel-bar-fill"
          style={{
            width: `${fill}%`,
            background: `linear-gradient(to right, ${color}, color-mix(in srgb, ${color}, white 20%))`,
            boxShadow: `0 0 10px ${glow}`,
          }}
        />
      </div>
      {reset && <span className="panel-bar-reset">Resets in {reset}</span>}
    </div>
  );
}

export function UsageView({ data, displayMode, onDisconnect }: UsageViewProps) {
  const sessionPercent = data.sessionPercentUsed;
  const weeklyPercent = data.weeklyPercentUsed;

  return (
    <div className="panel">
      <div className="panel-header">
        <img
          src={claudeMascot}
          alt="Claude"
          className="panel-logo"
          draggable={false}
        />
        {data.planType && (
          <span className="panel-plan">{data.planType}</span>
        )}
      </div>

      <div className="panel-bars">
        <UsageBar
          label="5-Hour Session"
          fill={computeFill(sessionPercent, displayMode)}
          color={getSessionColor(sessionPercent)}
          glow={getSessionGlow(sessionPercent)}
          displayMode={displayMode}
          reset={formatResetTime(data.sessionReset)}
        />

        <UsageBar
          label="Weekly"
          fill={computeFill(weeklyPercent, displayMode)}
          color={getWeeklyColor(weeklyPercent)}
          glow={getWeeklyGlow(weeklyPercent)}
          displayMode={displayMode}
          reset={formatResetTime(data.weeklyReset)}
        />

        {data.extraUsageSpend != null && (
          <div className="panel-extra">
            <span className="panel-extra-label">Extra Usage</span>
            <span className="panel-extra-value">
              ${(data.extraUsageSpend / 100).toFixed(2)}
              {data.extraUsageLimit != null && (
                <> / ${(data.extraUsageLimit / 100).toFixed(2)}</>
              )}
            </span>
          </div>
        )}
      </div>

      <button
        className="panel-link"
        onClick={() => openUrl("https://claude.ai/settings/usage")}
      >
        Open Dashboard
      </button>

      <button className="panel-disconnect" onClick={onDisconnect}>
        Disconnect
      </button>
    </div>
  );
}
