import type { ClaudeUsageData, DisplayMode } from "../types";
import {
  computeFill,
  formatResetTime,
  getSessionColor,
  getSessionGlow,
  getWeeklyColor,
  getWeeklyGlow,
} from "../utils";
import claudeMascot from "../assets/claude-mascot.png";

export function UsageView({
  data,
  displayMode,
  onDisconnect,
}: {
  data: ClaudeUsageData;
  displayMode: DisplayMode;
  onDisconnect: () => void;
}) {
  const sessionPercent = data.sessionPercentUsed;
  const weeklyPercent = data.weeklyPercentUsed;
  const sessionFill = computeFill(sessionPercent, displayMode);
  const weeklyFill = computeFill(weeklyPercent, displayMode);

  const sessionColor = getSessionColor(sessionPercent);
  const sessionGlow = getSessionGlow(sessionPercent);
  const weeklyColor = getWeeklyColor(weeklyPercent);
  const weeklyGlow = getWeeklyGlow(weeklyPercent);

  const sessionReset = formatResetTime(data.sessionReset);
  const weeklyReset = formatResetTime(data.weeklyReset);

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
        {/* 5-Hour Session bar */}
        <div className="panel-bar-group">
          <div className="panel-bar-label-row">
            <span className="panel-bar-name">5-Hour Session</span>
            <span className="panel-bar-pct">
              {parseFloat(sessionFill.toFixed(1))}%{" "}
              {displayMode === "remaining" ? "left" : "used"}
            </span>
          </div>
          <div className="panel-bar-track">
            <div
              className="panel-bar-fill"
              style={{
                width: `${sessionFill}%`,
                background: `linear-gradient(to right, ${sessionColor}, color-mix(in srgb, ${sessionColor}, white 20%))`,
                boxShadow: `0 0 10px ${sessionGlow}`,
              }}
            />
          </div>
          {sessionReset && (
            <span className="panel-bar-reset">Resets in {sessionReset}</span>
          )}
        </div>

        {/* Weekly bar */}
        <div className="panel-bar-group">
          <div className="panel-bar-label-row">
            <span className="panel-bar-name">Weekly</span>
            <span className="panel-bar-pct">
              {parseFloat(weeklyFill.toFixed(1))}%{" "}
              {displayMode === "remaining" ? "left" : "used"}
            </span>
          </div>
          <div className="panel-bar-track">
            <div
              className="panel-bar-fill"
              style={{
                width: `${weeklyFill}%`,
                background: `linear-gradient(to right, ${weeklyColor}, color-mix(in srgb, ${weeklyColor}, white 20%))`,
                boxShadow: `0 0 10px ${weeklyGlow}`,
              }}
            />
          </div>
          {weeklyReset && (
            <span className="panel-bar-reset">Resets in {weeklyReset}</span>
          )}
        </div>

        {/* Extra usage */}
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
        onClick={() => {
          import("@tauri-apps/plugin-opener").then((m) =>
            m.openUrl("https://claude.ai/settings/usage")
          );
        }}
      >
        Open Dashboard
      </button>

      <button className="panel-disconnect" onClick={onDisconnect}>
        Disconnect
      </button>
    </div>
  );
}
