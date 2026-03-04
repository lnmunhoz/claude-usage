import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import claudeLogo from "../assets/claude-logo.svg";

export function SetupView({ onSaved }: { onSaved: () => void }) {
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
      {error && <p className="setup-error">{error}</p>}
      {status === "waiting" ? (
        <>
          <div className="setup-waiting">
            <div className="loading-dot" />
            <span className="setup-waiting-text">Waiting for browser...</span>
          </div>
          <button
            className="panel-disconnect"
            onClick={() => setStatus("idle")}
          >
            Cancel
          </button>
        </>
      ) : (
        <button className="setup-save" onClick={handleLogin}>
          Login with Claude
        </button>
      )}
    </div>
  );
}
