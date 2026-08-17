import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNoteStore } from "../stores/noteStore";

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function SettingsPanel({ open, onClose }: Props) {
  const { closeToTray, darkMode, alarmInterval, alarmDuration, setCloseBehavior, setDarkMode, setAlarmInterval, setAlarmDuration } =
    useNoteStore();
  const [autoStart, setAutoStart] = useState(false);

  useEffect(() => {
    if (open) {
      invoke<boolean>("is_autostart_enabled_cmd")
        .then(setAutoStart)
        .catch(() => setAutoStart(false));
    }
  }, [open]);

  const toggleAutoStart = async () => {
    try {
      if (autoStart) {
        await invoke("disable_autostart_cmd");
        setAutoStart(false);
      } else {
        await invoke("enable_autostart_cmd");
        setAutoStart(true);
      }
    } catch (e) {
      window.alert(typeof e === "string" ? e : "切换开机自启失败");
      console.error("Failed to toggle autostart:", e);
    }
  };

  const toggleDarkMode = () => {
    setDarkMode(!darkMode);
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
          {/* Auto-start toggle */}
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

          {/* Dark mode toggle */}
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

          <label
            className="settings-row"
            onClick={() => setCloseBehavior(!closeToTray)}
          >
            <div className="settings-label">
              <div className="settings-label-title">关闭行为</div>
              <div className="settings-label-desc">
                {closeToTray
                  ? "点击关闭时最小化到托盘"
                  : "点击关闭时直接退出程序"}
              </div>
            </div>
            <div className={`settings-toggle ${closeToTray ? "on" : "off"}`}>
              <div className="settings-toggle-knob" />
            </div>
          </label>

          {/* Alarm mode params */}
          <div className="settings-section-title">闹钟提醒</div>
          <div className="settings-row">
            <div className="settings-label">
              <div className="settings-label-title">提醒间隔（分钟）</div>
              <div className="settings-label-desc">闹钟模式下每隔该时长（分钟）弹出置顶提醒</div>
            </div>
            <input
              type="number"
              min={1}
              max={1440}
              value={alarmInterval}
              className="settings-number-input"
              onChange={(e) => setAlarmInterval(Number(e.target.value))}
            />
          </div>
          <div className="settings-row">
            <div className="settings-label">
              <div className="settings-label-title">置顶时长（秒，最长 60）</div>
              <div className="settings-label-desc">置顶显示该时长后隐藏到任务栏</div>
            </div>
            <input
              type="number"
              min={1}
              max={60}
              value={alarmDuration}
              className="settings-number-input"
              onChange={(e) => setAlarmDuration(Number(e.target.value))}
            />
          </div>
        </div>
      </div>
    </>
  );
}
