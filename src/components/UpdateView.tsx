import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UpdateInfo } from "../types";
import appLogo from "../assets/app-logo.png";

export function UpdateView() {
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
        alt="Claude Usage"
        className="update-logo"
        draggable={false}
        data-tauri-drag-region
      />
      <h2 className="update-title" data-tauri-drag-region>
        Update Available
      </h2>
      <p className="update-version" data-tauri-drag-region>
        Claude Usage v{info.version}
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
