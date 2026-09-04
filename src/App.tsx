import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import TitleBar from "./components/TitleBar";
import Sidebar from "./components/Sidebar";
import NoteEditor from "./components/NoteEditor";
import SettingsPanel from "./components/SettingsPanel";
import UpdateDialog, { type UpdateState } from "./components/UpdateDialog";
import LixingDialog from "./components/LixingDialog";
import { useNoteStore } from "./stores/noteStore";
import type { UpdateInfo, WeeklyResult } from "./types";

function App() {
  const {
    fetchNotes,
    fetchCloseBehavior,
    fetchOpacity,
    fetchAlarmConfig,
    fetchSkin,
    fetchTheme,
    initDaily,
    getBalance,
    checkWeekly,
    opacity,
    skinImage,
  } = useNoteStore();
  const initialized = useRef(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);
  const [weeklyResult, setWeeklyResult] = useState<WeeklyResult | null>(null);
  const [settleToast, setSettleToast] = useState<string | null>(null);

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;

    const boot = async () => {
      // 1) Settle previous commission + auto-create today's.
      const daily = await initDaily();
      if (daily && (daily.settle_reward > 0 || daily.settle_mora > 0)) {
        setSettleToast(
          `前日委托已结算：+${daily.settle_reward} 原石 +${daily.settle_mora} 摩拉${
            daily.settle_triple ? "（三全完成）" : ""
          }`
        );
        setTimeout(() => setSettleToast(null), 6000);
      }
      // 2) Load notes, config, balance.
      fetchNotes();
      fetchCloseBehavior();
      fetchOpacity();
      fetchAlarmConfig();
      fetchSkin();
      fetchTheme();
      getBalance();
    };
    boot();

    // 非 Tauri（如纯浏览器预览）环境下无窗口桥接，跳过状态同步
    try {
      getCurrentWindow()
        .isAlwaysOnTop()
        .then((pinned) => {
          useNoteStore.setState({ pinMode: pinned ? "pin" : "normal" });
        })
        .catch(() => {});
    } catch {
      // getCurrentWindow() 同步抛错，忽略
    }
  }, []);

  // 回到前台时以后端为准同步一次原石余额，兜底其他场景造成的短线漂移
  useEffect(() => {
    let unlisten: Promise<() => void> | undefined;
    try {
      unlisten = getCurrentWindow().onFocusChanged(({ payload }) => {
        if (payload) getBalance();
      });
    } catch {
      // 非 Tauri 环境无窗口桥接
    }
    return () => {
      if (unlisten) unlisten.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleCheckUpdate = async () => {
    setUpdateState({ phase: "checking" });
    try {
      const info = await invoke<UpdateInfo>("check_update");
      setUpdateState({ phase: "result", info });
    } catch (e) {
      setUpdateState({
        phase: "error",
        message: typeof e === "string" ? e : "检查更新失败",
      });
    }
  };

  const handleUpdate = async () => {
    if (updateState?.phase !== "result") return;
    const { download_url, portable_download_url } = updateState.info;
    setUpdateState({ phase: "installing" });
    try {
      await invoke("download_and_install", {
        setupUrl: download_url,
        portableUrl: portable_download_url,
      });
      setUpdateState({ phase: "done" });
    } catch (e) {
      setUpdateState({
        phase: "error",
        message: typeof e === "string" ? e : "更新失败",
      });
    }
  };

  const handleCheckWeekly = async () => {
    try {
      const result = await checkWeekly();
      setWeeklyResult(result);
    } catch (e) {
      console.error("砺行修远结算失败:", e);
    }
  };

  // Global keyboard shortcuts
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod) return;

      const s = useNoteStore.getState();

      // Ctrl/Cmd + , → open settings
      if (!e.shiftKey && e.code === "Comma") {
        e.preventDefault();
        setSettingsOpen(true);
        return;
      }

      // Ctrl/Cmd + = → increase window opacity (layout-tolerant: ignore shift)
      if (e.code === "Equal") {
        e.preventDefault();
        s.setOpacity(Math.min(1, Number((s.opacity + 0.05).toFixed(2))));
        return;
      }

      // Ctrl/Cmd + - → decrease window opacity (layout-tolerant: ignore shift)
      if (e.code === "Minus") {
        e.preventDefault();
        s.setOpacity(Math.max(0.25, Number((s.opacity - 0.05).toFixed(2))));
        return;
      }

      // Ctrl/Cmd + Shift + T → cycle pin mode
      if (e.shiftKey && e.code === "KeyT") {
        e.preventDefault();
        s.cyclePinMode();
        return;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="app" style={{ "--app-bg-opacity": opacity } as React.CSSProperties}>
      {skinImage && (
        <>
          <div
            className="app-skin"
            style={{ backgroundImage: `url("${skinImage}")` }}
          />
          <div className="app-scrim" />
        </>
      )}
      <TitleBar
        onSettings={() => setSettingsOpen(true)}
        onCheckUpdate={handleCheckUpdate}
        onCheckWeekly={handleCheckWeekly}
      />
      <div className="app-body">
        <Sidebar />
        <main className="editor-area">
          <NoteEditor />
        </main>
      </div>

      {settleToast && (
        <div className="settle-toast">
          <span>{settleToast}</span>
          <button className="settle-toast-close" onClick={() => setSettleToast(null)}>
            ✕
          </button>
        </div>
      )}

      <SettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <UpdateDialog
        state={updateState}
        onClose={() => setUpdateState(null)}
        onUpdate={handleUpdate}
      />
      <LixingDialog result={weeklyResult} onClose={() => setWeeklyResult(null)} />
    </div>
  );
}

export default App;
