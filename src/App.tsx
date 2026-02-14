import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

// Poll interval in seconds (change this to test frequent fetches)
const POLL_INTERVAL_SECONDS = 5; // 5 minutes

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

// Plan bar: light indigo palette
function getPlanColor(percent: number): string {
  if (percent < 50) return "#818cf8"; // indigo-400
  if (percent < 75) return "#a78bfa"; // violet-400
  if (percent < 90) return "#f97316"; // orange
  return "#ef4444"; // red
}

function getPlanGlow(percent: number): string {
  if (percent < 50) return "rgba(129, 140, 248, 0.5)";
  if (percent < 75) return "rgba(167, 139, 250, 0.5)";
  if (percent < 90) return "rgba(249, 115, 22, 0.5)";
  return "rgba(239, 68, 68, 0.5)";
}

// On-demand bar: green-to-red palette
function getOdColor(percent: number): string {
  if (percent < 50) return "#22c55e"; // green
  if (percent < 75) return "#eab308"; // yellow
  if (percent < 90) return "#f97316"; // orange
  return "#ef4444"; // red
}

function getOdGlow(percent: number): string {
  if (percent < 50) return "rgba(34, 197, 94, 0.5)";
  if (percent < 75) return "rgba(234, 179, 8, 0.5)";
  if (percent < 90) return "rgba(249, 115, 22, 0.5)";
  return "rgba(239, 68, 68, 0.5)";
}

function App() {
  const [usage, setUsage] = useState<UsageData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshKey, setRefreshKey] = useState(0);
  const [animating, setAnimating] = useState(false);
  const isFirstLoad = useRef(true);

  const fetchUsage = useCallback(async () => {
    try {
      const data = await invoke<UsageData>("fetch_cursor_usage");
      console.log(data);
      setUsage(data);
      setError(null);

      // Trigger shimmer animation (skip on first load)
      if (!isFirstLoad.current) {
        setAnimating(true);
        setRefreshKey((k) => k + 1);
      }
      isFirstLoad.current = false;
    } catch (err) {
      console.error("fetch_cursor_usage error:", err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  // Clear animating state after animation completes
  useEffect(() => {
    if (!animating) return;
    const timer = setTimeout(() => setAnimating(false), 1100);
    return () => clearTimeout(timer);
  }, [animating, refreshKey]);

  useEffect(() => {
    fetchUsage();
    const interval = setInterval(fetchUsage, POLL_INTERVAL_SECONDS * 1000);
    return () => clearInterval(interval);
  }, [fetchUsage]);

  const planPercent = usage?.percentUsed ?? 0;
  const planFill = Math.min(100, Math.max(0, planPercent));
  const planColor = getPlanColor(planPercent);
  const planGlow = getPlanGlow(planPercent);

  const odPercent = usage?.onDemandPercentUsed ?? 0;
  const odFill = Math.min(100, Math.max(0, odPercent));
  const odColor = getOdColor(odPercent);
  const odGlow = getOdGlow(odPercent);

  const hasOnDemand =
    usage?.onDemandLimitUsd != null && usage.onDemandLimitUsd > 0;

  // Total combined percentage across plan + on-demand
  const totalUsed = (usage?.usedUsd ?? 0) + (usage?.onDemandUsedUsd ?? 0);
  const totalLimit = (usage?.limitUsd ?? 0) + (usage?.onDemandLimitUsd ?? 0);
  const totalPercent = totalLimit > 0 ? (totalUsed / totalLimit) * 100 : 0;

  const shimmerClass = animating ? "shimmer" : "";
  const bounceClass = animating ? "bounce" : "";

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
      ) : (
        <>
          <div className="bars-row" data-tauri-drag-region>
            {/* Plan usage bar */}
            <div className="bar-column" data-tauri-drag-region>
              <div className="bar-track" data-tauri-drag-region>
                <div
                  key={`plan-${refreshKey}`}
                  className={`bar-fill ${shimmerClass}`}
                  data-tauri-drag-region
                  style={{
                    height: `${planFill}%`,
                    backgroundColor: planColor,
                    boxShadow: `0 0 10px ${planGlow}, 0 0 4px ${planGlow}`,
                  }}
                />
              </div>
              <span
                key={`plan-label-${refreshKey}`}
                className={`bar-label ${bounceClass}`}
                data-tauri-drag-region
              >
                {Math.round(planPercent)}%
              </span>
              <span className="bar-tag" data-tauri-drag-region>
                P
              </span>
            </div>

            {/* On-demand usage bar */}
            {hasOnDemand && (
              <div className="bar-column" data-tauri-drag-region>
                <div className="bar-track" data-tauri-drag-region>
                  <div
                    key={`od-${refreshKey}`}
                    className={`bar-fill ${shimmerClass}`}
                    data-tauri-drag-region
                    style={{
                      height: `${odFill}%`,
                      backgroundColor: odColor,
                      boxShadow: `0 0 10px ${odGlow}, 0 0 4px ${odGlow}`,
                    }}
                  />
                </div>
                <span
                  key={`od-label-${refreshKey}`}
                  className={`bar-label ${bounceClass}`}
                  data-tauri-drag-region
                >
                  {Math.round(odPercent)}%
                </span>
                <span className="bar-tag" data-tauri-drag-region>
                  D
                </span>
              </div>
            )}
          </div>

          <div
            key={`total-${refreshKey}`}
            className={`total-percent ${bounceClass}`}
            data-tauri-drag-region
          >
            {Math.round(totalPercent)}%
          </div>
          {usage?.membershipType && (
            <div className="plan-label" data-tauri-drag-region>
              {usage.membershipType}
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default App;
