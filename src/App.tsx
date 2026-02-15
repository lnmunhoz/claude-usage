import { useEffect, useState, useCallback, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";

// Poll interval in seconds

const POLL_INTERVAL_SECONDS = 60;

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

interface Settings {
  showPlan: boolean;
  showOnDemand: boolean;
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
  primaryBar: BarConfig | null;
  secondaryBar: BarConfig | null;
  showBothBars: boolean;
  totalPercent: number | null; // null = don't show total row
  planLabel: string | null; // membership type / plan name
  spendDelta: string | null; // floating cost delta (Cursor only)
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
function getClaudeColor(p: number) {
  if (p < 50) return "#f59e0b";
  if (p < 75) return "#f97316";
  if (p < 90) return "#ef4444";
  return "#dc2626";
}
function getClaudeGlow(p: number) {
  if (p < 50) return "rgba(245, 158, 11, 0.5)";
  if (p < 75) return "rgba(249, 115, 22, 0.5)";
  if (p < 90) return "rgba(239, 68, 68, 0.5)";
  return "rgba(220, 38, 38, 0.5)";
}

function clampFill(p: number) {
  return Math.min(100, Math.max(0, p));
}

// ---------------------------------------------------------------------------
// Adapter: Cursor → BarViewModel
// ---------------------------------------------------------------------------

function cursorToViewModel(
  data: UsageData,
  settings: { showPlan: boolean; showOnDemand: boolean },
  delta: string | null,
): BarViewModel {
  const planPercent = data.percentUsed;
  const hasOnDemand =
    data.onDemandLimitUsd != null && data.onDemandLimitUsd > 0;
  const odPercent = data.onDemandPercentUsed;

  const primaryBar: BarConfig | null = settings.showPlan
    ? {
        percent: planPercent,
        fill: clampFill(planPercent),
        label: "P",
        color: getPlanColor(planPercent),
        glow: getPlanGlow(planPercent),
      }
    : null;

  const secondaryBar: BarConfig | null =
    settings.showOnDemand && hasOnDemand
      ? {
          percent: odPercent,
          fill: clampFill(odPercent),
          label: "D",
          color: getOdColor(odPercent),
          glow: getOdGlow(odPercent),
        }
      : null;

  const showBothBars = primaryBar != null && secondaryBar != null;

  let totalPercent: number | null = null;
  if (showBothBars) {
    const totalUsed =
      (settings.showPlan ? data.usedUsd : 0) +
      (settings.showOnDemand ? data.onDemandUsedUsd : 0);
    const totalLimit =
      (settings.showPlan ? data.limitUsd : 0) +
      (settings.showOnDemand ? (data.onDemandLimitUsd ?? 0) : 0);
    totalPercent = totalLimit > 0 ? (totalUsed / totalLimit) * 100 : 0;
  }

  return {
    primaryBar,
    secondaryBar,
    showBothBars,
    totalPercent,
    planLabel: data.membershipType ?? null,
    spendDelta: delta,
  };
}

// ---------------------------------------------------------------------------
// Adapter: Claude → BarViewModel
// ---------------------------------------------------------------------------

function claudeToViewModel(data: ClaudeUsageData): BarViewModel {
  const sessionPercent = data.sessionPercentUsed;
  const weeklyPercent = data.weeklyPercentUsed;

  return {
    primaryBar: {
      percent: sessionPercent,
      fill: clampFill(sessionPercent),
      label: "S",
      color: getClaudeColor(sessionPercent),
      glow: getClaudeGlow(sessionPercent),
    },
    secondaryBar: {
      percent: weeklyPercent,
      fill: clampFill(weeklyPercent),
      label: "W",
      color: getClaudeColor(weeklyPercent),
      glow: getClaudeGlow(weeklyPercent),
    },
    showBothBars: true,
    totalPercent: null, // Claude doesn't show a combined total
    planLabel: data.planType ?? "claude",
    spendDelta: null, // Claude doesn't track dollar spend deltas
  };
}

// ---------------------------------------------------------------------------
// App component
// ---------------------------------------------------------------------------

function App() {
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

  // --- Cursor settings ---
  const [showPlan, setShowPlan] = useState(true);
  const [showOnDemand, setShowOnDemand] = useState(true);

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

        if (sessionDelta > 0.01) setPlanPulsing(true);
        if (weeklyDelta > 0.01) setOdPulsing(true);

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
        }

        if (planDelta > 0.001) setPlanPulsing(true);
        if (odDelta > 0.001) setOdPulsing(true);

        prevPlanUsed.current = data.usedUsd;
        prevOdUsed.current = data.onDemandUsedUsd;
      }

      if (!isFirstLoad.current) {
        setAnimating(true);
        setRefreshKey((k) => k + 1);
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
  }, [animating, refreshKey]);

  useEffect(() => {
    if (!spendDelta) return;
    const timer = setTimeout(() => setSpendDelta(null), 3700);
    return () => clearTimeout(timer);
  }, [spendDelta, refreshKey]);

  useEffect(() => {
    if (!planPulsing && !odPulsing) return;
    const timer = setTimeout(() => {
      setPlanPulsing(false);
      setOdPulsing(false);
    }, 1500);
    return () => clearTimeout(timer);
  }, [planPulsing, odPulsing, refreshKey]);

  useEffect(() => {
    try {
      const label = getCurrentWindow().label;
      setProvider(label === "claude" ? "claude" : "cursor");
    } catch {
      setProvider("cursor");
    }
  }, []);

  useEffect(() => {
    if (!provider) return;
    fetchUsage();
    const interval = setInterval(fetchUsage, POLL_INTERVAL_SECONDS * 1000);
    return () => clearInterval(interval);
  }, [fetchUsage, provider]);

  useEffect(() => {
    if (provider === "claude") return;
    invoke<Settings>("get_settings").then((s) => {
      setShowPlan(s.showPlan);
      setShowOnDemand(s.showOnDemand);
    });

    const unlisten = listen<Settings>("settings-changed", (event) => {
      setShowPlan(event.payload.showPlan);
      setShowOnDemand(event.payload.showOnDemand);
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
      return claudeToViewModel(claudeUsage);
    }
    if (provider === "cursor" && cursorUsage) {
      return cursorToViewModel(
        cursorUsage,
        { showPlan, showOnDemand },
        spendDelta,
      );
    }
    return null;
  }, [provider, cursorUsage, claudeUsage, showPlan, showOnDemand, spendDelta]);

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
                      backgroundColor: vm.primaryBar.color,
                      boxShadow: `0 0 10px ${vm.primaryBar.glow}, 0 0 4px ${vm.primaryBar.glow}`,
                    }}
                  />
                </div>
                <span
                  key={`plan-label-${refreshKey}`}
                  className={`bar-label ${bounceClass}`}
                  data-tauri-drag-region
                >
                  {vm.primaryBar.percent.toFixed(1)}%
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
                      backgroundColor: vm.secondaryBar.color,
                      boxShadow: `0 0 10px ${vm.secondaryBar.glow}, 0 0 4px ${vm.secondaryBar.glow}`,
                    }}
                  />
                </div>
                <span
                  key={`od-label-${refreshKey}`}
                  className={`bar-label ${bounceClass}`}
                  data-tauri-drag-region
                >
                  {vm.secondaryBar.percent.toFixed(1)}%
                </span>
                {vm.showBothBars && (
                  <span className="bar-tag" data-tauri-drag-region>
                    {vm.secondaryBar.label}
                  </span>
                )}
              </div>
            )}
          </div>

          {vm.totalPercent != null && (
            <div
              key={`total-${refreshKey}`}
              className={`total-percent ${bounceClass}`}
              data-tauri-drag-region
            >
              {vm.totalPercent.toFixed(1)}%
            </div>
          )}

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
