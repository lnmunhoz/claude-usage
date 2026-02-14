import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

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

function getBarColor(percent: number): string {
  if (percent < 50) return "#22c55e"; // green
  if (percent < 75) return "#eab308"; // yellow
  if (percent < 90) return "#f97316"; // orange
  return "#ef4444"; // red
}

function getBarGlow(percent: number): string {
  if (percent < 50) return "rgba(34, 197, 94, 0.4)";
  if (percent < 75) return "rgba(234, 179, 8, 0.4)";
  if (percent < 90) return "rgba(249, 115, 22, 0.4)";
  return "rgba(239, 68, 68, 0.4)";
}

function App() {
  const [usage, setUsage] = useState<UsageData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchUsage = useCallback(async () => {
    try {
      const data = await invoke<UsageData>("fetch_cursor_usage");
      console.log(data);
      setUsage(data);
      setError(null);
    } catch (err) {
      console.error("fetch_cursor_usage error:", err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchUsage();
    const interval = setInterval(fetchUsage, 5 * 60 * 1000);
    return () => clearInterval(interval);
  }, [fetchUsage]);

  const planPercent = usage?.percentUsed ?? 0;
  const planFill = Math.min(100, Math.max(0, planPercent));
  const planColor = getBarColor(planPercent);
  const planGlow = getBarGlow(planPercent);

  const odPercent = usage?.onDemandPercentUsed ?? 0;
  const odFill = Math.min(100, Math.max(0, odPercent));
  const odColor = getBarColor(odPercent);
  const odGlow = getBarGlow(odPercent);

  const hasOnDemand = usage?.onDemandLimitUsd != null && usage.onDemandLimitUsd > 0;

  // Total combined percentage across plan + on-demand
  const totalUsed = (usage?.usedUsd ?? 0) + (usage?.onDemandUsedUsd ?? 0);
  const totalLimit = (usage?.limitUsd ?? 0) + (usage?.onDemandLimitUsd ?? 0);
  const totalPercent = totalLimit > 0 ? (totalUsed / totalLimit) * 100 : 0;

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
                  className="bar-fill"
                  data-tauri-drag-region
                  style={{
                    height: `${planFill}%`,
                    backgroundColor: planColor,
                    boxShadow: `0 0 8px ${planGlow}`,
                  }}
                />
              </div>
              <span className="bar-label" data-tauri-drag-region>
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
                    className="bar-fill"
                    data-tauri-drag-region
                    style={{
                      height: `${odFill}%`,
                      backgroundColor: odColor,
                      boxShadow: `0 0 8px ${odGlow}`,
                    }}
                  />
                </div>
                <span className="bar-label" data-tauri-drag-region>
                  {Math.round(odPercent)}%
                </span>
                <span className="bar-tag" data-tauri-drag-region>
                  D
                </span>
              </div>
            )}
          </div>

          <div className="total-percent" data-tauri-drag-region>
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
