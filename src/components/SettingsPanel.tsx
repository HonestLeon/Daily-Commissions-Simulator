import { useState, useEffect } from "react";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function SettingsPanel({ open, onClose }: Props) {
  const [autoStart, setAutoStart] = useState(false);
  const [darkMode, setDarkMode] = useState(() => {
    return document.documentElement.dataset.theme === "dark";
  });

  useEffect(() => {
    if (open) {
      isEnabled().then(setAutoStart).catch(() => setAutoStart(false));
    }
  }, [open]);

  const toggleAutoStart = async () => {
    try {
      if (autoStart) {
        await disable();
      } else {
        await enable();
      }
      setAutoStart(!autoStart);
    } catch (e) {
      console.error("Failed to toggle autostart:", e);
    }
  };

  const toggleDarkMode = () => {
    const next = !darkMode;
    setDarkMode(next);
    document.documentElement.dataset.theme = next ? "dark" : "light";
  };

  if (!open) return null;

  return (
    <>
      <div className="settings-overlay" onClick={onClose} />
      <div className="settings-panel">
        <div className="settings-header">
          <span>设置</span>
          <button className="settings-close" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="settings-body">
          <label className="settings-row" onClick={toggleAutoStart}>
            <div className="settings-label">
              <div className="settings-label-title">开机自启动</div>
              <div className="settings-label-desc">
                登录 Windows 时自动启动应用（最小化到托盘）
              </div>
            </div>
            <div className={`settings-toggle ${autoStart ? "on" : "off"}`}>
              <div className="settings-toggle-knob" />
            </div>
          </label>

          <label className="settings-row" onClick={toggleDarkMode}>
            <div className="settings-label">
              <div className="settings-label-title">暗色模式</div>
              <div className="settings-label-desc">
                切换深色/浅色主题外观
              </div>
            </div>
            <div className={`settings-toggle ${darkMode ? "on" : "off"}`}>
              <div className="settings-toggle-knob" />
            </div>
          </label>
        </div>
      </div>
    </>
  );
}
