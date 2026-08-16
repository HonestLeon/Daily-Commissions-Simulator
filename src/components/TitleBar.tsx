import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useNoteStore } from "../stores/noteStore";
import FirstCloseDialog from "./FirstCloseDialog";
import yuanShiIcon from "../assets/yuan-shi.png";

interface Props {
  onSettings: () => void;
  onCheckUpdate: () => void;
  onCheckWeekly: () => void;
}

export default function TitleBar({ onSettings, onCheckUpdate, onCheckWeekly }: Props) {
  const { isPinned, opacity, closeToTray, setCloseBehavior, togglePin, setOpacity, yuanShi } =
    useNoteStore();
  const [showCloseDialog, setShowCloseDialog] = useState(false);

  const handlePin = async () => {
    const win = getCurrentWindow();
    const newPinned = !isPinned;
    await win.setAlwaysOnTop(newPinned);
    togglePin();
  };

  const handleOpacityChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setOpacity(Number(e.target.value));
  };

  const handleMinimize = async () => {
    await getCurrentWindow().minimize();
  };

  const handleClose = async () => {
    if (closeToTray === null) {
      setShowCloseDialog(true);
    } else if (closeToTray) {
      await getCurrentWindow().hide();
    } else {
      await getCurrentWindow().close();
    }
  };

  const handleCloseChoice = async (tray: boolean) => {
    setShowCloseDialog(false);
    await setCloseBehavior(tray);
    if (tray) {
      await getCurrentWindow().hide();
    } else {
      await getCurrentWindow().close();
    }
  };

  return (
    <>
      <div
        className="titlebar"
        data-tauri-drag-region
        onDoubleClick={(e) => {
          if ((e.target as HTMLElement).hasAttribute("data-tauri-drag-region")) {
            const win = getCurrentWindow();
            win.isMaximized().then((max) => {
              if (max) win.unmaximize();
              else win.maximize();
            });
          }
        }}
      >
        <span className="titlebar-title">轻量便签</span>
        <div className="titlebar-yuanshi" title="原石余额">
          <img src={yuanShiIcon} alt="原石" width={18} height={18} />
          {yuanShi}
        </div>
        <div style={{ flex: 1 }} data-tauri-drag-region />
        <div className="titlebar-controls">
          <button
            className="titlebar-btn titlebar-btn-text"
            onClick={onCheckWeekly}
            title="砺行修远"
          >
            砺行修远
          </button>

          <button
            className={`titlebar-btn ${isPinned ? "active" : ""}`}
            onClick={handlePin}
            title={isPinned ? "取消置顶" : "窗口置顶"}
          >
            📌
          </button>

          <button className="titlebar-btn" onClick={onCheckUpdate} title="检查更新">
            🔄
          </button>

          <div className="titlebar-slider" title={`透明度: ${Math.round(opacity * 100)}%`}>
            <span style={{ fontSize: 12 }}>🎚</span>
            <input
              type="range"
              min="0.25"
              max="1"
              step="0.01"
              value={opacity}
              onChange={handleOpacityChange}
              style={{ width: 60, cursor: "pointer" }}
            />
            <span className="titlebar-opacity-value">{Math.round(opacity * 100)}%</span>
          </div>

          <button className="titlebar-btn" onClick={onSettings} title="设置">
            ⚙
          </button>

          <button className="titlebar-btn" onClick={handleMinimize} title="最小化到任务栏">
            ─
          </button>

          <button className="titlebar-btn titlebar-btn-close" onClick={handleClose} title="关闭">
            ✕
          </button>
        </div>
      </div>

      <FirstCloseDialog open={showCloseDialog} onSelect={handleCloseChoice} />
    </>
  );
}
