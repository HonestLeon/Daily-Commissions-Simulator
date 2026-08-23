import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNoteStore } from "../stores/noteStore";
import type { AppTheme } from "../types";

const THEME_OPTIONS: { id: AppTheme; name: string; bg: string; accent: string }[] = [
  { id: "light", name: "默认", bg: "#ffffff", accent: "#4a9eff" },
  { id: "sky", name: "天空蓝", bg: "#d2e2f6", accent: "#3d8bfd" },
  { id: "sakura", name: "樱花粉", bg: "#f6dde8", accent: "#d95f97" },
  { id: "green", name: "护眼绿", bg: "#d2e8d0", accent: "#4a9e6f" },
];

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function SettingsPanel({ open, onClose }: Props) {
  const {
    closeToTray,
    alarmInterval,
    alarmDuration,
    opacity,
    skinImage,
    skinActive,
    theme,
    setCloseBehavior,
    setAlarmInterval,
    setAlarmDuration,
    setOpacity,
    importSkin,
    removeSkin,
    setTheme,
  } = useNoteStore();
  const [autoStart, setAutoStart] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

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

  const handleSkinFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = ""; // 允许重复选择同一文件
    if (!file) return;
    try {
      const dataUrl = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(reader.result as string);
        reader.onerror = () => reject(reader.error);
        reader.readAsDataURL(file);
      });
      await importSkin(dataUrl);
    } catch (err) {
      window.alert(typeof err === "string" ? err : "导入皮肤失败");
      console.error("Failed to import skin:", err);
    }
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
          <button
            type="button"
            role="switch"
            aria-checked={autoStart}
            className="settings-row"
            onClick={toggleAutoStart}
          >
            <div className="settings-label">
              <div className="settings-label-title">开机自启动</div>
              <div className="settings-label-desc">
                登录 Windows 时自动启动应用（最小化到托盘）
              </div>
            </div>
            <div className={`settings-toggle ${autoStart ? "on" : "off"}`}>
              <div className="settings-toggle-knob" />
            </div>
          </button>

          {/* Theme selection */}
          <div className="settings-section-title">主题</div>
          <div
            className="theme-options"
            role="radiogroup"
            aria-label="界面主题"
          >
            {THEME_OPTIONS.map((t) => (
              <button
                key={t.id}
                type="button"
                role="radio"
                aria-checked={theme === t.id}
                className={`theme-card${theme === t.id ? " active" : ""}`}
                onClick={() => setTheme(t.id)}
              >
                <div
                  className="theme-card-preview"
                  style={{ background: t.bg }}
                >
                  <span className="theme-card-sample">委托 Aa</span>
                  <span
                    className="theme-card-bar"
                    style={{ background: t.accent }}
                  />
                </div>
                <span className="theme-card-name">{t.name}</span>
              </button>
            ))}
          </div>

          {/* Window opacity */}
          <div className="settings-section-title">显示</div>
          <label className="settings-row">
            <div className="settings-label">
              <div className="settings-label-title">
                透明度（{Math.round(opacity * 100)}%）
              </div>
              <div className="settings-label-desc">
                窗口整体不透明度（25% ~ 100%）
              </div>
            </div>
            <input
              type="range"
              className="settings-range"
              min="0.25"
              max="1"
              step="0.01"
              value={opacity}
              onChange={(e) => setOpacity(Number(e.target.value))}
              aria-label="窗口透明度"
            />
          </label>

          {/* Skin section */}
          <div className="settings-section-title">皮肤</div>
          <div className="settings-row">
            <div className="settings-label">
              <div className="settings-label-title">导入皮肤</div>
              <div className="settings-label-desc">
                整窗背景图（仅浅色模式生效，支持 PNG / JPG / WebP，≤8MB）
              </div>
            </div>
            {skinImage && (
              <div
                className="settings-skin-preview"
                style={{ backgroundImage: `url(${skinImage})` }}
              />
            )}
          </div>
          <div className="settings-row">
            <button
              className="settings-action-btn"
              onClick={() => fileInputRef.current?.click()}
            >
              导入 / 更换
            </button>
            {skinActive && (
              <button
                className="settings-action-btn danger"
                onClick={removeSkin}
              >
                移除皮肤
              </button>
            )}
          </div>
          <input
            ref={fileInputRef}
            type="file"
            accept=".png,.jpg,.jpeg,.webp,image/png,image/jpeg,image/webp"
            style={{ display: "none" }}
            onChange={handleSkinFile}
          />

          <button
            type="button"
            role="switch"
            aria-checked={!!closeToTray}
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
          </button>

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
