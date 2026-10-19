import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import TitleBar from "./components/TitleBar";
import PetStage from "./components/PetStage";
import Sidebar from "./components/Sidebar";
import NoteEditor from "./components/NoteEditor";
import SettingsPanel from "./components/SettingsPanel";
import UpdateDialog, { type UpdateState } from "./components/UpdateDialog";
import LixingDialog from "./components/LixingDialog";
import { useNoteStore, avoidancePollMs } from "./stores/noteStore";
import type { UpdateInfo, WeeklyResult } from "./types";

function App() {
  const {
    fetchNotes,
    fetchCloseBehavior,
    fetchOpacity,
    fetchAlarmConfig,
    fetchSkin,
    fetchTheme,
    fetchUiState,
    autosaveGeometry,
    fetchProgress,
    initDaily,
    getBalance,
    checkWeekly,
    opacity,
    skinImage,
    form,
    formBusy,
    petEnabled,
    petHideOnFullscreen,
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
      // 0) 先恢复形态与窗口几何（window-state 已不再记几何，靠我们自己落位）
      await fetchUiState();
      // 1) Settle previous commission + auto-create today's.
      const daily = await initDaily();
      const petMode = useNoteStore.getState().form === "pet";
      if (daily && (daily.settle_reward > 0 || daily.settle_mora > 0)) {
        const text = `前日委托已结算：+${daily.settle_reward} 原石 +${daily.settle_mora} 摩拉${
          daily.settle_triple ? "（三全完成）" : ""
        }`;
        if (petMode) {
          // 宠物态下结算浮层是隐藏的，这句话改由气泡窗承载
          void useNoteStore.getState().say(text, 12_000);
        } else {
          setSettleToast(text);
          setTimeout(() => setSettleToast(null), 6000);
        }
      }
      // 2) Load notes, config, balance.
      fetchNotes();
      fetchCloseBehavior();
      fetchOpacity();
      fetchAlarmConfig();
      fetchSkin();
      fetchTheme();
      getBalance();
      await fetchProgress();
      if (petMode) {
        const p = useNoteStore.getState().progress;
        if (p?.note_found && !p.tomorrow_filled) {
          void useNoteStore.getState().say("主人，今天的明日委托还空着哦～", 10_000);
        }
      }
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

  // 托盘里切换「桌宠模式」后，Rust 只改配置并广播，形态与几何仍由前端的 fetchUiState 落位
  useEffect(() => {
    let un: (() => void) | undefined;
    let dead = false;
    listen("ui-state-changed", () => {
      void fetchUiState();
    })
      .then((fn) => {
        if (dead) fn();
        else un = fn;
      })
      .catch(() => {}); // 非 Tauri 环境无事件总线
    return () => {
      dead = true;
      un?.();
    };
  }, [fetchUiState]);

  // 窗口被隐藏/显示（托盘、关闭到托盘、避让）时暂停或恢复解码。
  // 这些 hide/show 分散在 Rust 与 store 多处，靠 Rust 统一广播一个可见性事件。
  useEffect(() => {
    let un: (() => void) | undefined;
    let dead = false;
    listen<boolean>("pet-visibility-changed", ({ payload }) => {
      useNoteStore.getState().setMediaPaused(!payload);
    })
      .then((fn) => {
        if (dead) fn();
        else un = fn;
      })
      .catch(() => {}); // 非 Tauri 环境无事件总线
    return () => {
      dead = true;
      un?.();
    };
  }, []);

  // 他人全屏时桌宠让路：挡游戏/挡投屏是桌宠差评第一源。只在宠物态轮询。
  useEffect(() => {
    if (!petEnabled || !petHideOnFullscreen || form !== "pet") return;
    let dead = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const tick = async () => {
      await useNoteStore.getState().checkFullscreenAvoidance();
      if (dead) return;
      timer = setTimeout(() => void tick(), avoidancePollMs());
    };
    void tick(); // 立刻查一次：不然宠物会先挡住全屏画面几秒才躲
    return () => {
      dead = true;
      if (timer) clearTimeout(timer);
    };
  }, [petEnabled, petHideOnFullscreen, form]);

  // 设置面板 / 更新弹窗 / 砺行弹窗都是便签态界面：宠物态下它们会开在
  // visibility:hidden 的 .note-root 里，看起来像「点了没反应」。集中兜底而不是在
  // 每个入口各插一道门禁——后台自动弹出的那两条（更新检查、周结算）本来就没有入口。
  useEffect(() => {
    if (form !== "pet" || formBusy) return;
    if (!settingsOpen && !updateState && !weeklyResult) return;
    void useNoteStore.getState().setForm("note");
  }, [form, formBusy, settingsOpen, updateState, weeklyResult]);

  // 用户手动拖动/缩放窗口后记录几何：window-state 已不再保存 SIZE/POSITION，不自己存就丢
  useEffect(() => {
    let unlisten: Promise<() => void>[] = [];
    let dead = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const onGeometryChange = () => {
      if (timer) clearTimeout(timer);
      // 拖窗时 onMoved 每帧都来，攒 400ms 再落盘
      timer = setTimeout(() => void autosaveGeometry(), 400);
    };
    try {
      const win = getCurrentWindow();
      unlisten.push(win.onMoved(onGeometryChange), win.onResized(onGeometryChange));
    } catch {
      // 非 Tauri 环境
    }
    return () => {
      dead = true;
      if (timer) clearTimeout(timer);
      unlisten.forEach((p) => p.then((fn) => !dead && fn()));
    };
  }, [autosaveGeometry]);

  // 鼠标移开便签若干毫秒后自动缩回宠物；宽限期可配，0 = 不自动缩回。
  // 用 pointerleave/pointerenter 判定，不轮询光标位置（§5.2 的 Win10 黑屏诱因）。
  useEffect(() => {
    if (!petEnabled || form !== "note") return;
    const autoMs = useNoteStore.getState().petAutoCollapseMs;
    if (autoMs <= 0) return;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const cancel = () => {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
    };
    const onLeave = () => {
      cancel();
      timer = setTimeout(() => void useNoteStore.getState().setForm("pet"), autoMs);
    };
    const root = document.documentElement;
    root.addEventListener("pointerleave", onLeave);
    root.addEventListener("pointerenter", cancel);
    // 刚按下鼠标说明用户在操作，别收（真正禁缩回的门禁在 setForm 里）
    document.addEventListener("pointerdown", cancel, true);
    return () => {
      cancel();
      root.removeEventListener("pointerleave", onLeave);
      root.removeEventListener("pointerenter", cancel);
      document.removeEventListener("pointerdown", cancel, true);
    };
  }, [petEnabled, form]);

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
        // 设置面板是便签态的界面：宠物态下先展开再开，否则会弹在看不见的位置
        void s.setForm("note");
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

      // Ctrl/Cmd + Shift + P → 便签 ⇄ 桌宠
      if (e.shiftKey && e.code === "KeyP") {
        e.preventDefault();
        void s.setForm(s.form === "pet" ? "note" : "pet");
        return;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div
      className="app"
      data-form={form}
      style={{ "--app-bg-opacity": opacity } as React.CSSProperties}
    >
      {skinImage && (
        <>
          <div className="app-skin" style={{ backgroundImage: `url("${skinImage}")` }} />
          <div className="app-scrim" />
        </>
      )}
      {/* 形态切换只靠 CSS 折叠、绝不卸载：Milkdown 重建会重跑初始化回调（AGENTS.md），
          而且光标与滚动位置都会丢 */}
      <div className="note-root">
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
      </div>
      {petEnabled && form === "pet" && <PetStage />}

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
