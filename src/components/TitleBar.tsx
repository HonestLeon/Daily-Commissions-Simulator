import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useNoteStore } from "../stores/noteStore";
import FirstCloseDialog from "./FirstCloseDialog";
import {
  IconPin,
  IconAlarm,
  IconNormal,
  IconRefresh,
  IconSettings,
  IconMinimize,
  IconClose,
} from "./titleIcons";
import yuanShiIcon from "../assets/yuan-shi.png";
import moraIcon from "../assets/mora.png";
import qiyuanIcon from "../assets/qiyuan.png";
import appIcon from "../assets/app-icon.png";
import lixingIcon from "../assets/lixing.png";

const GITEE_REPO_URL = "https://gitee.com/Honest_Leon/lightweight-sticky-notes";

interface Props {
  onSettings: () => void;
  onCheckUpdate: () => void;
  onCheckWeekly: () => void;
}

export default function TitleBar({ onSettings, onCheckUpdate, onCheckWeekly }: Props) {
  const {
    pinMode,
    alarmInterval,
    alarmDuration,
    closeToTray,
    setCloseBehavior,
    cyclePinMode,
    yuanShi,
    mora,
  } = useNoteStore();
  const [showCloseDialog, setShowCloseDialog] = useState(false);

  const ModeIcon = pinMode === "pin" ? IconPin : pinMode === "alarm" ? IconAlarm : IconNormal;
  const modeTitle =
    pinMode === "pin"
      ? "始终置顶（点击切换模式）"
      : pinMode === "alarm"
        ? `定时置顶：每 ${alarmInterval} 分钟弹出 ${alarmDuration}s 后隐藏到任务栏（点击切换模式）`
        : "正常状态（点击切换为始终置顶）";

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
        <img
          className="titlebar-logo"
          src={appIcon}
          alt="每日委托模拟器"
          title="打开 Gitee 仓库"
          onClick={() => openUrl(GITEE_REPO_URL)}
        />
        <div className="titlebar-yuanshi" title="原石余额">
          <img src={yuanShiIcon} alt="" width={18} height={18} aria-hidden="true" />
          {yuanShi}
        </div>
        <div className="titlebar-mora" title="摩拉余额">
          <img src={moraIcon} alt="" width={18} height={18} aria-hidden="true" />
          {mora}
        </div>
        <div style={{ flex: 1 }} data-tauri-drag-region />
        <div className="titlebar-controls">
          <button
            className="titlebar-btn"
            onClick={onCheckWeekly}
            title="砺行修远"
            aria-label="砺行修远"
          >
            <img className="titlebar-btn-icon" src={lixingIcon} alt="" aria-hidden="true" />
          </button>

          {/* 祈愿：功能占位，暂未实现 */}
          <button className="titlebar-btn" title="祈愿" aria-label="祈愿">
            <img
              className="titlebar-btn-icon"
              src={qiyuanIcon}
              alt=""
              style={{ width: 20, height: 20 }}
              aria-hidden="true"
            />
          </button>

          <button
            className={`titlebar-btn ${pinMode === "pin" ? "active" : pinMode === "alarm" ? "alarm" : ""}`}
            onClick={() => cyclePinMode()}
            title={modeTitle}
            aria-label={modeTitle}
          >
            <ModeIcon />
          </button>

          <button
            className="titlebar-btn"
            onClick={onCheckUpdate}
            title="检查更新"
            aria-label="检查更新"
          >
            <IconRefresh />
          </button>

          <button className="titlebar-btn" onClick={onSettings} title="设置" aria-label="设置">
            <IconSettings />
          </button>

          <button
            className="titlebar-btn"
            onClick={handleMinimize}
            title="最小化到任务栏"
            aria-label="最小化到任务栏"
          >
            <IconMinimize />
          </button>

          <button
            className="titlebar-btn titlebar-btn-close"
            onClick={handleClose}
            title="关闭"
            aria-label="关闭"
          >
            <IconClose />
          </button>
        </div>
      </div>

      <FirstCloseDialog open={showCloseDialog} onSelect={handleCloseChoice} />
    </>
  );
}
