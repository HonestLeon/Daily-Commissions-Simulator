import { useState, useEffect } from "react";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useNoteStore } from "../stores/noteStore";

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function SettingsPanel({ open, onClose }: Props) {
  const { templates, deleteTemplate, fetchTemplates } = useNoteStore();
  const [autoStart, setAutoStart] = useState(false);
  const [darkMode, setDarkMode] = useState(() => {
    return document.documentElement.dataset.theme === "dark";
  });

  useEffect(() => {
    if (open) {
      isEnabled().then(setAutoStart).catch(() => setAutoStart(false));
      fetchTemplates();
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

  const customTemplates = templates.filter((t) => !t.preset);
  const presetTemplates = templates.filter((t) => t.preset);

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

          <hr className="settings-divider" />

          {/* Template management */}
          <div className="settings-section-title">模板管理</div>

          {presetTemplates.length > 0 && (
            <div className="settings-template-group">
              <div className="settings-template-group-label">预设模板</div>
              {presetTemplates.map((t) => (
                <div key={t.id} className="settings-template-item">
                  <span>📋 {t.name}</span>
                </div>
              ))}
            </div>
          )}

          {customTemplates.length > 0 && (
            <div className="settings-template-group">
              <div className="settings-template-group-label">自定义模板</div>
              {customTemplates.map((t) => (
                <div key={t.id} className="settings-template-item">
                  <span>📝 {t.name}</span>
                  <button
                    className="settings-template-delete"
                    onClick={() => deleteTemplate(t.id)}
                    title="删除模板"
                  >
                    🗑
                  </button>
                </div>
              ))}
            </div>
          )}

          {templates.length === 0 && (
            <div className="settings-label-desc" style={{ padding: "8px 0" }}>
              暂无模板。右键便签可保存为模板。
            </div>
          )}
        </div>
      </div>
    </>
  );
}
