import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";

import claudeLogo from "./assets/claude-logo.svg";

// ---------------------------------------------------------------------------
// Raw data interfaces (match Rust backend structs)
// ---------------------------------------------------------------------------

interface ClaudeUsageData {
  sessionPercentUsed: number;
  weeklyPercentUsed: number;
  sessionReset: string | null;
  weeklyReset: string | null;
  planType: string | null;
  extraUsageSpend: number | null;
  extraUsageLimit: number | null;
}

type DisplayMode = "usage" | "remaining";

interface Settings {
  displayMode: DisplayMode;
  pollIntervalSeconds: number;
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

function getSessionColor(p: number) {
  if (p < 50) return "#facc15";
  if (p < 75) return "#eab308";
  if (p < 90) return "#f97316";
  return "#ef4444";
}
function getSessionGlow(p: number) {
  if (p < 50) return "rgba(250, 204, 21, 0.5)";
  if (p < 75) return "rgba(234, 179, 8, 0.5)";
  if (p < 90) return "rgba(249, 115, 22, 0.5)";
  return "rgba(239, 68, 68, 0.5)";
}
function getWeeklyColor(p: number) {
  if (p < 50) return "#f97316";
  if (p < 75) return "#ea580c";
  if (p < 90) return "#ef4444";
  return "#dc2626";
}
function getWeeklyGlow(p: number) {
  if (p < 50) return "rgba(249, 115, 22, 0.5)";
  if (p < 75) return "rgba(234, 88, 12, 0.5)";
  if (p < 90) return "rgba(239, 68, 68, 0.5)";
  return "rgba(220, 38, 38, 0.5)";
}

function clampFill(p: number) {
  return Math.min(100, Math.max(0, p));
}

function computeFill(percent: number, mode: DisplayMode): number {
  return mode === "remaining" ? clampFill(100 - percent) : clampFill(percent);
}

// ---------------------------------------------------------------------------
// Format reset countdown
// ---------------------------------------------------------------------------

function formatResetTime(isoString: string | null): string | null {
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

// ---------------------------------------------------------------------------
// Settings view (inline)
// ---------------------------------------------------------------------------

function SettingsView({ onBack }: { onBack: () => void }) {
  const [value, setValue] = useState(60);
  const [unit, setUnit] = useState<"seconds" | "minutes" | "hours">("seconds");
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    invoke<Settings>("get_settings").then((s) => {
      const totalSeconds = s.pollIntervalSeconds ?? 60;
      if (totalSeconds >= 3600 && totalSeconds % 3600 === 0) {
        setValue(totalSeconds / 3600);
        setUnit("hours");
      } else if (totalSeconds >= 60 && totalSeconds % 60 === 0) {
        setValue(totalSeconds / 60);
        setUnit("minutes");
      } else {
        setValue(totalSeconds);
        setUnit("seconds");
      }
      setLoaded(true);
    });
  }, []);

  const handleSave = async () => {
    await invoke("save_poll_interval", {
      intervalValue: value,
      intervalUnit: unit,
    });
    onBack();
  };

  if (!loaded) return null;

  return (
    <div className="settings-view">
      <label className="settings-label">Refresh every</label>
      <div className="settings-row">
        <input
          type="number"
          min={1}
          className="settings-input"
          value={value}
          onChange={(e) => setValue(Math.max(1, parseInt(e.target.value) || 1))}
        />
        <select
          className="settings-select"
          value={unit}
          onChange={(e) =>
            setUnit(e.target.value as "seconds" | "minutes" | "hours")
          }
        >
          <option value="seconds">seconds</option>
          <option value="minutes">minutes</option>
          <option value="hours">hours</option>
        </select>
      </div>
      <button className="settings-save" onClick={handleSave}>
        Save
      </button>
      <button className="settings-back" onClick={onBack}>
        Cancel
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Update dialog view
// ---------------------------------------------------------------------------

import appLogo from "./assets/app-logo.png";

interface UpdateInfo {
  version: string;
  body: string;
}

function UpdateView() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);

  useEffect(() => {
    const unlisten = listen<UpdateInfo>("update-info", (event) => {
      setInfo(event.payload);
    });
    getCurrentWindow().emit("update-ready", {});
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleUpdate = async () => {
    await getCurrentWindow().emit("update-response", { accepted: true });
  };

  const handleSkip = async () => {
    await getCurrentWindow().emit("update-response", { accepted: false });
  };

  if (!info) {
    return (
      <div className="update-view">
        <div className="update-loading">Checking for updates...</div>
      </div>
    );
  }

  return (
    <div className="update-view" data-tauri-drag-region>
      <img
        src={appLogo}
        alt="Token Juice"
        className="update-logo"
        draggable={false}
        data-tauri-drag-region
      />
      <h2 className="update-title" data-tauri-drag-region>
        Update Available
      </h2>
      <p className="update-version" data-tauri-drag-region>
        Token Juice v{info.version}
      </p>
      {info.body && (
        <div className="update-notes" data-tauri-drag-region>
          <p className="update-notes-label">Release Notes</p>
          <div className="update-notes-content">{info.body}</div>
        </div>
      )}
      <div className="update-buttons">
        <button className="update-btn update-btn-primary" onClick={handleUpdate}>
          Update & Restart
        </button>
        <button className="update-btn update-btn-secondary" onClick={handleSkip}>
          Not Now
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Token Setup view
// ---------------------------------------------------------------------------

function SetupView({ onSaved }: { onSaved: () => void }) {
  const [status, setStatus] = useState<"idle" | "waiting" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  const handleLogin = async () => {
    setStatus("waiting");
    setError(null);
    try {
      await invoke("login_oauth");
      onSaved();
    } catch (err) {
      setError(String(err));
      setStatus("error");
    }
  };

  return (
    <div className="setup-view">
      <img
        src={claudeLogo}
        alt="Claude"
        className="setup-logo"
        draggable={false}
      />
      <h2 className="setup-title">Connect Claude</h2>
      <p className="setup-subtitle">
        Sign in with your Anthropic account to view your usage.
      </p>
      {status === "waiting" && (
        <div className="setup-waiting">
          <div className="loading-dot" />
          <span className="setup-waiting-text">Waiting for browser...</span>
        </div>
      )}
      {error && <p className="setup-error">{error}</p>}
      <button
        className="setup-save"
        onClick={handleLogin}
        disabled={status === "waiting"}
      >
        {status === "waiting" ? "Waiting for browser..." : "Login with Claude"}
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Usage panel view
// ---------------------------------------------------------------------------

function UsageView({
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
          src={claudeLogo}
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
        Open Claude Usage
      </button>

      <button className="panel-disconnect" onClick={onDisconnect}>
        Disconnect
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// App component
// ---------------------------------------------------------------------------

type AppMode = "setup" | "usage" | "settings" | "update";

function App() {
  const [mode, setMode] = useState<AppMode>("setup");
  const [claudeUsage, setClaudeUsage] = useState<ClaudeUsageData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [displayMode, setDisplayMode] = useState<DisplayMode>("remaining");
  const [pollIntervalSeconds, setPollIntervalSeconds] = useState(60);
  const isFirstLoad = useRef(true);

  // Check if this is the update window
  useEffect(() => {
    try {
      const label = getCurrentWindow().label;
      if (label === "update") {
        setMode("update");
        setLoading(false);
        return;
      }
    } catch {
      // not update window
    }

    // Check for existing token
    invoke<boolean>("has_token").then((hasToken) => {
      if (hasToken) {
        setMode("usage");
      } else {
        setMode("setup");
        setLoading(false);
      }
    });
  }, []);

  // Load settings
  useEffect(() => {
    invoke<Settings>("get_settings").then((s) => {
      setDisplayMode(s.displayMode ?? "remaining");
      setPollIntervalSeconds(s.pollIntervalSeconds ?? 60);
    });

    const unlistenSettings = listen<Settings>("settings-changed", (event) => {
      setDisplayMode(event.payload.displayMode ?? "remaining");
      setPollIntervalSeconds(event.payload.pollIntervalSeconds ?? 60);
    });

    const unlistenShowSettings = listen("show-settings", () => {
      setMode("settings");
    });

    return () => {
      unlistenSettings.then((fn) => fn());
      unlistenShowSettings.then((fn) => fn());
    };
  }, []);

  // Fetch usage data
  const fetchUsage = useCallback(async () => {
    if (mode !== "usage") return;
    try {
      const data = await invoke<ClaudeUsageData>("fetch_claude_usage");
      setClaudeUsage(data);
      setError(null);
      if (isFirstLoad.current) {
        isFirstLoad.current = false;
      }
      // Update menu bar tray text with session percentage left
      const pctLeft = (100 - data.sessionPercentUsed).toFixed(0);
      await invoke("update_tray_title", { title: `${pctLeft}%` });
    } catch (err) {
      console.error("usage fetch error:", err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [mode]);

  useEffect(() => {
    if (mode !== "usage") return;
    setLoading(true);
    fetchUsage();
    const interval = setInterval(fetchUsage, pollIntervalSeconds * 1000);
    return () => clearInterval(interval);
  }, [fetchUsage, pollIntervalSeconds, mode]);

  // Handle disconnect
  const handleDisconnect = async () => {
    try {
      await invoke("clear_token");
    } catch {
      // ignore
    }
    await invoke("update_tray_title", { title: null }).catch(() => {});
    setClaudeUsage(null);
    setError(null);
    setLoading(true);
    isFirstLoad.current = true;
    setMode("setup");
    setLoading(false);
  };

  // Handle token saved
  const handleTokenSaved = () => {
    setMode("usage");
    setLoading(true);
    isFirstLoad.current = true;
  };

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  if (mode === "update") {
    return <UpdateView />;
  }

  if (mode === "setup") {
    return <SetupView onSaved={handleTokenSaved} />;
  }

  if (mode === "settings") {
    return <SettingsView onBack={() => setMode("usage")} />;
  }

  // Usage mode
  return (
    <div className="panel-container">
      {loading ? (
        <div className="panel-loading">
          <div className="loading-dot" />
          <span className="loading-text">Loading usage...</span>
        </div>
      ) : error ? (
        <div className="panel-error">
          <span className="error-icon">!</span>
          <p className="error-text">{error}</p>
          <button className="error-retry" onClick={fetchUsage}>
            Retry
          </button>
          <button className="panel-disconnect" onClick={handleDisconnect}>
            Disconnect
          </button>
        </div>
      ) : claudeUsage ? (
        <UsageView
          data={claudeUsage}
          displayMode={displayMode}
          onDisconnect={handleDisconnect}
        />
      ) : null}
    </div>
  );
}

export default App;
