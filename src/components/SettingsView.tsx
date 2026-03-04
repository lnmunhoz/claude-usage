import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types";

export function SettingsView({ onBack }: { onBack: () => void }) {
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
