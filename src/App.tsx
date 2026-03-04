import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";

import type { ClaudeUsageData, DisplayMode, Settings } from "./types";
import { SkeletonUsageView } from "./components/SkeletonUsageView";
import { UsageView } from "./components/UsageView";
import { SetupView } from "./components/SetupView";
import { SettingsView } from "./components/SettingsView";
import { UpdateView } from "./components/UpdateView";

// Detect if running inside Tauri webview vs regular browser
const isTauri = () =>
  typeof window !== "undefined" &&
  !!((window as any).__TAURI_INTERNALS__ || (window as any).__TAURI__);

// ---------------------------------------------------------------------------
// App component
// ---------------------------------------------------------------------------

type AppMode = "loading" | "setup" | "usage" | "settings" | "update";

function App() {
  const [mode, setMode] = useState<AppMode>("loading");
  const [claudeUsage, setClaudeUsage] = useState<ClaudeUsageData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [displayMode, setDisplayMode] = useState<DisplayMode>("remaining");
  const [tauriAvailable] = useState(isTauri);
  const hasReceivedDataRef = useRef(false);

  // Check if this is the update window
  useEffect(() => {
    if (!tauriAvailable) {
      setLoading(false);
      return;
    }

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
  }, [tauriAvailable]);

  // Load settings
  useEffect(() => {
    if (!tauriAvailable) return;

    invoke<Settings>("get_settings").then((s) => {
      setDisplayMode(s.displayMode ?? "remaining");
    });

    const unlistenSettings = listen<Settings>("settings-changed", (event) => {
      setDisplayMode(event.payload.displayMode ?? "remaining");
    });

    const unlistenShowSettings = listen("show-settings", () => {
      setMode("settings");
    });

    return () => {
      unlistenSettings.then((fn) => fn());
      unlistenShowSettings.then((fn) => fn());
    };
  }, [tauriAvailable]);

  // Listen for usage data from the backend poller
  useEffect(() => {
    if (mode !== "usage" || !tauriAvailable) return;

    // Immediate fetch — the backend poller's initial event may have
    // fired before this listener was set up (race condition on startup).
    invoke<ClaudeUsageData>("fetch_claude_usage")
      .then((data) => {
        setClaudeUsage(data);
        hasReceivedDataRef.current = true;
        setError(null);
        setLoading(false);
      })
      .catch((err) => {
        console.error("initial usage fetch error:", err);
        if (!hasReceivedDataRef.current) {
          setError(String(err));
          setLoading(false);
        }
      });

    const unlistenUsage = listen<ClaudeUsageData>("usage-updated", (event) => {
      setClaudeUsage(event.payload);
      hasReceivedDataRef.current = true;
      setError(null);
      setLoading(false);
    });

    const unlistenError = listen<{ message: string }>("usage-error", (event) => {
      console.error("usage poll error:", event.payload.message);
      // Only show error if we have no data yet (don't replace good data with error)
      if (!hasReceivedDataRef.current) {
        setError(event.payload.message);
        setLoading(false);
      }
    });

    return () => {
      hasReceivedDataRef.current = false;
      unlistenUsage.then((fn) => fn());
      unlistenError.then((fn) => fn());
    };
  }, [mode, tauriAvailable]);

  // Manual retry for error state
  const fetchUsage = useCallback(async () => {
    if (mode !== "usage") return;
    setLoading(true);
    try {
      const data = await invoke<ClaudeUsageData>("fetch_claude_usage");
      setClaudeUsage(data);
      setError(null);
    } catch (err) {
      console.error("usage fetch error:", err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [mode]);

  // Handle disconnect
  const handleDisconnect = async () => {
    try {
      // clear_token also stops the backend poller and clears the tray title
      await invoke("clear_token");
    } catch {
      // ignore
    }
    await invoke("update_tray_title", { title: null }).catch(() => {});
    setClaudeUsage(null);
    setError(null);
    setMode("setup");
  };

  // Handle token saved (login_oauth starts the backend poller automatically)
  const handleTokenSaved = () => {
    setMode("usage");
    setLoading(true);
  };

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  if (!tauriAvailable) {
    return (
      <div className="panel-error">
        <span className="error-icon">!</span>
        <p className="error-text">
          This app requires the Tauri desktop runtime.
          <br />
          Run with <code>cargo tauri dev</code> instead of opening in a browser.
        </p>
      </div>
    );
  }

  if (mode === "update") {
    return <UpdateView />;
  }

  if (mode === "loading") {
    return <SkeletonUsageView />;
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
        <SkeletonUsageView />
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
