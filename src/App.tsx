import { useEffect, useState, useCallback, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";

import cursorLogo from "./assets/cursor-logo.svg";
import claudeLogo from "./assets/claude-logo.svg";

// Test mode: simulates a fake spend delta on every poll to test animations
const TEST_MODE = false;
const FAKE_DELTA_USD = 1.5;

// ---------------------------------------------------------------------------
// Raw data interfaces (match Rust backend structs)
// ---------------------------------------------------------------------------

interface UsageData {
  percentUsed: number;
  usedUsd: number;
  limitUsd: number;
  remainingUsd: number;
  onDemandPercentUsed: number;
  onDemandUsedUsd: number;
  onDemandLimitUsd: number | null;
  billingCycleEnd: string | null;
  membershipType: string | null;
}

interface ClaudeUsageData {
  sessionPercentUsed: number;
  weeklyPercentUsed: number;
  sessionReset: string | null;
  weeklyReset: string | null;
  planType: string | null;
  extraUsageSpend: number | null;
  extraUsageLimit: number | null;
}

type Provider = "cursor" | "claude";

type DisplayMode = "usage" | "remaining";

interface Settings {
  showPlan: boolean;
  showOnDemand: boolean;
  displayMode: DisplayMode;
  pollIntervalSeconds: number;
}

// ---------------------------------------------------------------------------
// Unified view model — the rendering layer only sees this
// ---------------------------------------------------------------------------

interface BarConfig {
  percent: number;
  fill: number; // clamped 0-100
  label: string; // "P", "D", "S", "W"
  color: string;
  glow: string;
}

interface BarViewModel {
  logo: string; // imported SVG path
  primaryBar: BarConfig | null;
  secondaryBar: BarConfig | null;
  showBothBars: boolean;
  planLabel: string | null; // membership type / plan name
  spendDelta: string | null; // floating cost delta (Cursor only)
  displayMode: DisplayMode; // "usage" or "remaining"
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

function getPlanColor(p: number) {
  if (p < 50) return "#818cf8";
  if (p < 75) return "#a78bfa";
  if (p < 90) return "#f97316";
  return "#ef4444";
}
function getPlanGlow(p: number) {
  if (p < 50) return "rgba(129, 140, 248, 0.5)";
  if (p < 75) return "rgba(167, 139, 250, 0.5)";
  if (p < 90) return "rgba(249, 115, 22, 0.5)";
  return "rgba(239, 68, 68, 0.5)";
}
function getOdColor(p: number) {
  if (p < 50) return "#22c55e";
  if (p < 75) return "#eab308";
  if (p < 90) return "#f97316";
  return "#ef4444";
}
function getOdGlow(p: number) {
  if (p < 50) return "rgba(34, 197, 94, 0.5)";
  if (p < 75) return "rgba(234, 179, 8, 0.5)";
  if (p < 90) return "rgba(249, 115, 22, 0.5)";
  return "rgba(239, 68, 68, 0.5)";
}
// Claude session (5h) — yellowish
function getClaudeSessionColor(p: number) {
  if (p < 50) return "#facc15";
  if (p < 75) return "#eab308";
  if (p < 90) return "#f97316";
  return "#ef4444";
}
function getClaudeSessionGlow(p: number) {
  if (p < 50) return "rgba(250, 204, 21, 0.5)";
  if (p < 75) return "rgba(234, 179, 8, 0.5)";
  if (p < 90) return "rgba(249, 115, 22, 0.5)";
  return "rgba(239, 68, 68, 0.5)";
}
// Claude weekly (7d) — orangeish
function getClaudeWeeklyColor(p: number) {
  if (p < 50) return "#f97316";
  if (p < 75) return "#ea580c";
  if (p < 90) return "#ef4444";
  return "#dc2626";
}
function getClaudeWeeklyGlow(p: number) {
  if (p < 50) return "rgba(249, 115, 22, 0.5)";
  if (p < 75) return "rgba(234, 88, 12, 0.5)";
  if (p < 90) return "rgba(239, 68, 68, 0.5)";
  return "rgba(220, 38, 38, 0.5)";
}

function clampFill(p: number) {
  return Math.min(100, Math.max(0, p));
}

// ---------------------------------------------------------------------------
// Adapter: Cursor → BarViewModel
// ---------------------------------------------------------------------------

function computeFill(percent: number, mode: DisplayMode): number {
  return mode === "remaining" ? clampFill(100 - percent) : clampFill(percent);
}

function cursorToViewModel(
  data: UsageData,
  settings: {
    showPlan: boolean;
    showOnDemand: boolean;
    displayMode: DisplayMode;
  },
  delta: string | null,
): BarViewModel {
  const planPercent = data.percentUsed;
  const hasOnDemand =
    data.onDemandLimitUsd != null && data.onDemandLimitUsd > 0;
  const odPercent = data.onDemandPercentUsed;

  const primaryBar: BarConfig | null = settings.showPlan
    ? {
        percent: computeFill(planPercent, settings.displayMode),
        fill: computeFill(planPercent, settings.displayMode),
        label: "P",
        color: getPlanColor(planPercent),
        glow: getPlanGlow(planPercent),
      }
    : null;

  const secondaryBar: BarConfig | null =
    settings.showOnDemand && hasOnDemand
      ? {
          percent: computeFill(odPercent, settings.displayMode),
          fill: computeFill(odPercent, settings.displayMode),
          label: "D",
          color: getOdColor(odPercent),
          glow: getOdGlow(odPercent),
        }
      : null;

  const showBothBars = primaryBar != null && secondaryBar != null;

  return {
    logo: cursorLogo,
    primaryBar,
    secondaryBar,
    showBothBars,
    planLabel: data.membershipType ?? null,
    spendDelta: delta,
    displayMode: settings.displayMode,
  };
}

// ---------------------------------------------------------------------------
// Adapter: Claude → BarViewModel
// ---------------------------------------------------------------------------

function claudeToViewModel(
  data: ClaudeUsageData,
  mode: DisplayMode,
): BarViewModel {
  const sessionPercent = data.sessionPercentUsed;
  const weeklyPercent = data.weeklyPercentUsed;

  return {
    logo: claudeLogo,
    primaryBar: {
      percent: computeFill(sessionPercent, mode),
      fill: computeFill(sessionPercent, mode),
      label: "5h",
      color: getClaudeSessionColor(sessionPercent),
      glow: getClaudeSessionGlow(sessionPercent),
    },
    secondaryBar: {
      percent: computeFill(weeklyPercent, mode),
      fill: computeFill(weeklyPercent, mode),
      label: "Week",
      color: getClaudeWeeklyColor(weeklyPercent),
      glow: getClaudeWeeklyGlow(weeklyPercent),
    },
    showBothBars: true,
    planLabel: data.planType ?? "claude",
    spendDelta: null, // Claude doesn't track dollar spend deltas
    displayMode: mode,
  };
}

// ---------------------------------------------------------------------------
// Settings view — rendered when window label is "settings"
// ---------------------------------------------------------------------------

function SettingsView() {
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
          onChange={(e) => setUnit(e.target.value as "seconds" | "minutes" | "hours")}
        >
          <option value="seconds">seconds</option>
          <option value="minutes">minutes</option>
          <option value="hours">hours</option>
        </select>
      </div>
      <button className="settings-save" onClick={handleSave}>
        Save
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// App component
// ---------------------------------------------------------------------------

function App() {
  // --- Window mode ---
  const [windowMode, setWindowMode] = useState<"usage" | "settings">("usage");

  // --- Provider detection ---
  const [provider, setProvider] = useState<Provider | null>(null);

  // --- Raw data from backend ---
  const [cursorUsage, setCursorUsage] = useState<UsageData | null>(null);
  const [claudeUsage, setClaudeUsage] = useState<ClaudeUsageData | null>(null);

  // --- UI state ---
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshKey, setRefreshKey] = useState(0);
  const [animating, setAnimating] = useState(false);
  const [spendDelta, setSpendDelta] = useState<string | null>(null);
  const [planPulsing, setPlanPulsing] = useState(false);
  const [odPulsing, setOdPulsing] = useState(false);

  // --- Settings ---
  const [showPlan, setShowPlan] = useState(true);
  const [showOnDemand, setShowOnDemand] = useState(true);
  const [displayMode, setDisplayMode] = useState<DisplayMode>("remaining");
  const [pollIntervalSeconds, setPollIntervalSeconds] = useState(60);

  // --- Delta tracking refs ---
  const isFirstLoad = useRef(true);
  const prevPlanUsed = useRef<number | null>(null);
  const prevOdUsed = useRef<number | null>(null);
  const prevClaudeSession = useRef<number | null>(null);
  const prevClaudeWeekly = useRef<number | null>(null);

  // -------------------------------------------------------------------------
  // Fetch — provider-specific, but isolated to data only
  // -------------------------------------------------------------------------

  const fetchUsage = useCallback(async () => {
    if (!provider) return;

    try {
      if (provider === "claude") {
        const data = await invoke<ClaudeUsageData>("fetch_claude_usage");
        setClaudeUsage(data);
        setCursorUsage(null);
        setError(null);

        const prevSession = prevClaudeSession.current;
        const prevWeekly = prevClaudeWeekly.current;
        const sessionDelta =
          prevSession !== null ? data.sessionPercentUsed - prevSession : 0;
        const weeklyDelta =
          prevWeekly !== null ? data.weeklyPercentUsed - prevWeekly : 0;

        if (sessionDelta > 0.01 || weeklyDelta > 0.01) {
          if (sessionDelta > 0.01) setPlanPulsing(true);
          if (weeklyDelta > 0.01) setOdPulsing(true);
          if (!isFirstLoad.current) {
            setAnimating(true);
            setRefreshKey((k) => k + 1);
          }
        }

        prevClaudeSession.current = data.sessionPercentUsed;
        prevClaudeWeekly.current = data.weeklyPercentUsed;
        setSpendDelta(null);
      } else {
        const data = await invoke<UsageData>("fetch_cursor_usage");
        setCursorUsage(data);
        setClaudeUsage(null);
        setError(null);

        const prevPlan = prevPlanUsed.current;
        const prevOd = prevOdUsed.current;
        let planDelta = prevPlan !== null ? data.usedUsd - prevPlan : 0;
        let odDelta = prevOd !== null ? data.onDemandUsedUsd - prevOd : 0;
        let totalDelta = planDelta + odDelta;

        if (TEST_MODE && !isFirstLoad.current && totalDelta < 0.001) {
          const fakePlan = Math.random() > 0.5;
          planDelta = fakePlan ? FAKE_DELTA_USD : 0;
          odDelta = fakePlan ? 0 : FAKE_DELTA_USD;
          totalDelta = FAKE_DELTA_USD;
        }

        if (totalDelta > 0.001) {
          setSpendDelta(`-$${totalDelta.toFixed(2)}`);
          if (planDelta > 0.001) setPlanPulsing(true);
          if (odDelta > 0.001) setOdPulsing(true);
          if (!isFirstLoad.current) {
            setAnimating(true);
            setRefreshKey((k) => k + 1);
          }
        }

        prevPlanUsed.current = data.usedUsd;
        prevOdUsed.current = data.onDemandUsedUsd;
      }

      isFirstLoad.current = false;
    } catch (err) {
      console.error("usage fetch error:", err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [provider]);

  // -------------------------------------------------------------------------
  // Effects
  // -------------------------------------------------------------------------

  useEffect(() => {
    if (!animating) return;
    const timer = setTimeout(() => setAnimating(false), 1100);
    return () => clearTimeout(timer);
  }, [animating]);

  useEffect(() => {
    if (!spendDelta) return;
    const timer = setTimeout(() => setSpendDelta(null), 3700);
    return () => clearTimeout(timer);
  }, [spendDelta]);

  useEffect(() => {
    if (!planPulsing && !odPulsing) return;
    const timer = setTimeout(() => {
      setPlanPulsing(false);
      setOdPulsing(false);
    }, 1500);
    return () => clearTimeout(timer);
  }, [planPulsing, odPulsing]);

  useEffect(() => {
    try {
      const label = getCurrentWindow().label;
      if (label === "settings") {
        setWindowMode("settings");
      } else {
        setProvider(label === "claude" ? "claude" : "cursor");
      }
    } catch {
      setProvider("cursor");
    }
  }, []);

  useEffect(() => {
    if (!provider) return;
    fetchUsage();
    const interval = setInterval(fetchUsage, pollIntervalSeconds * 1000);
    return () => clearInterval(interval);
  }, [fetchUsage, provider, pollIntervalSeconds]);

  useEffect(() => {
    invoke<Settings>("get_settings").then((s) => {
      setShowPlan(s.showPlan);
      setShowOnDemand(s.showOnDemand);
      setDisplayMode(s.displayMode ?? "remaining");
      setPollIntervalSeconds(s.pollIntervalSeconds ?? 60);
    });

    const unlisten = listen<Settings>("settings-changed", (event) => {
      setShowPlan(event.payload.showPlan);
      setShowOnDemand(event.payload.showOnDemand);
      setDisplayMode(event.payload.displayMode ?? "remaining");
      setPollIntervalSeconds(event.payload.pollIntervalSeconds ?? 60);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [provider]);

  // -------------------------------------------------------------------------
  // Build view model — single place where provider data meets the view
  // -------------------------------------------------------------------------

  const vm: BarViewModel | null = useMemo(() => {
    if (provider === "claude" && claudeUsage) {
      return claudeToViewModel(claudeUsage, displayMode);
    }
    if (provider === "cursor" && cursorUsage) {
      return cursorToViewModel(
        cursorUsage,
        { showPlan, showOnDemand, displayMode },
        spendDelta,
      );
    }
    return null;
  }, [
    provider,
    cursorUsage,
    claudeUsage,
    showPlan,
    showOnDemand,
    displayMode,
    spendDelta,
  ]);

  // -------------------------------------------------------------------------
  // Animation classes
  // -------------------------------------------------------------------------

  const shimmerClass = animating ? "shimmer" : "";
  const bounceClass = animating ? "bounce" : "";
  const planPulseClass = planPulsing ? "pulse-glow" : "";
  const odPulseClass = odPulsing ? "pulse-glow" : "";

  // -------------------------------------------------------------------------
  // Render — NO provider-specific branching below this line
  // -------------------------------------------------------------------------

  if (windowMode === "settings") {
    return <SettingsView />;
  }

  return (
    <div className="widget" data-tauri-drag-region>
      {loading ? (
        <div className="loading-indicator" data-tauri-drag-region>
          <div className="loading-dot" />
        </div>
      ) : error ? (
        <div className="error-indicator" data-tauri-drag-region title={error}>
          <span className="error-icon">!</span>
        </div>
      ) : vm ? (
        <>
          <img
            src={vm.logo}
            alt=""
            className="widget-logo"
            data-tauri-drag-region
            draggable={false}
          />
          <div className="bars-row" data-tauri-drag-region>
            {vm.spendDelta && (
              <span
                key={`spend-${refreshKey}`}
                className="spend-float"
                data-tauri-drag-region
              >
                {vm.spendDelta}
              </span>
            )}

            {/* Primary bar */}
            {vm.primaryBar && (
              <div className="bar-column" data-tauri-drag-region>
                <div className="bar-track" data-tauri-drag-region>
                  <div
                    key={`plan-${refreshKey}`}
                    className={`bar-fill ${shimmerClass} ${planPulseClass}`}
                    data-tauri-drag-region
                    style={{
                      height: `${vm.primaryBar.fill}%`,
                      background: `linear-gradient(to top, ${vm.primaryBar.color} 0%, ${vm.primaryBar.color} 30%, color-mix(in srgb, ${vm.primaryBar.color}, white 25%) 50%, ${vm.primaryBar.color} 70%, ${vm.primaryBar.color} 100%)`,
                      backgroundSize: "100% 300%",
                      boxShadow: `0 0 14px ${vm.primaryBar.glow}, 0 0 6px ${vm.primaryBar.glow}, inset 0 0 8px rgba(255,255,255,0.1)`,
                    }}
                  />
                </div>
                <span
                  key={`plan-label-${refreshKey}`}
                  className={`bar-label ${bounceClass}`}
                  data-tauri-drag-region
                >
                  {parseFloat(vm.primaryBar.percent.toFixed(1))}%
                </span>
                {vm.showBothBars && (
                  <span className="bar-tag" data-tauri-drag-region>
                    {vm.primaryBar.label}
                  </span>
                )}
              </div>
            )}

            {/* Secondary bar */}
            {vm.secondaryBar && (
              <div className="bar-column" data-tauri-drag-region>
                <div className="bar-track" data-tauri-drag-region>
                  <div
                    key={`od-${refreshKey}`}
                    className={`bar-fill ${shimmerClass} ${odPulseClass}`}
                    data-tauri-drag-region
                    style={{
                      height: `${vm.secondaryBar.fill}%`,
                      background: `linear-gradient(to top, ${vm.secondaryBar.color} 0%, ${vm.secondaryBar.color} 30%, color-mix(in srgb, ${vm.secondaryBar.color}, white 25%) 50%, ${vm.secondaryBar.color} 70%, ${vm.secondaryBar.color} 100%)`,
                      backgroundSize: "100% 300%",
                      boxShadow: `0 0 14px ${vm.secondaryBar.glow}, 0 0 6px ${vm.secondaryBar.glow}, inset 0 0 8px rgba(255,255,255,0.1)`,
                    }}
                  />
                </div>
                <span
                  key={`od-label-${refreshKey}`}
                  className={`bar-label ${bounceClass}`}
                  data-tauri-drag-region
                >
                  {parseFloat(vm.secondaryBar.percent.toFixed(1))}%
                </span>
                {vm.showBothBars && (
                  <span className="bar-tag" data-tauri-drag-region>
                    {vm.secondaryBar.label}
                  </span>
                )}
              </div>
            )}
          </div>

          {vm.planLabel && (
            <div className="plan-label" data-tauri-drag-region>
              {vm.planLabel}
            </div>
          )}
        </>
      ) : null}
    </div>
  );
}

export default App;
