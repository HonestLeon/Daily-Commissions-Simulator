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
import {
  useNoteStore,
  avoidancePollMs,
  isCursorOnWindow,
  isMouseButtonDown,
} from "./stores/noteStore";
import type { InitDailyResult, UpdateInfo, WeeklyResult } from "./types";

/** 本地日期键（YYYY-MM-DD），用于跨午夜检测；不能用 UTC 日期 */
function localDateKey(date = new Date()): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(
    date.getDate()
  ).padStart(2, "0")}`;
}

// 一次「鼠标移开」最多向 OS 求证几次，每次隔 autoMs：默认宽限下约 10 秒，够最慢的一次
// 拖窗或拉边框。问满就作罢，不再空转——光标长期停在窗口边框上不是罕见姿势。
const COLLAPSE_PROBE_MAX = 20;

function App() {
  const {
    fetchNotes,
    fetchOpacity,
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
    petAutoCollapseMs,
  } = useNoteStore();
  const initialized = useRef(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);
  const [weeklyResult, setWeeklyResult] = useState<WeeklyResult | null>(null);
  const [settleToast, setSettleToast] = useState<string | null>(null);

  /** 结算提示：宠物态走气泡，便签态走浮层；返回是否真的有结算要播报 */
  const announceSettlement = (daily: InitDailyResult | null): boolean => {
    if (!daily || (daily.settle_reward <= 0 && daily.settle_mora <= 0)) return false;
    const text = `前日委托已结算：+${daily.settle_reward} 原石 +${daily.settle_mora} 摩拉${
      daily.settle_triple ? "（三全完成）" : ""
    }`;
    if (useNoteStore.getState().form === "pet") {
      // 宠物态下结算浮层是隐藏的，这句话改由气泡窗承载
      void useNoteStore.getState().say(text, 12_000);
    } else {
      setSettleToast(text);
      setTimeout(() => setSettleToast(null), 6000);
    }
    return true;
  };

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;

    const boot = async () => {
      // 0) 主窗以 visible:false 创建、由 Rust 的 setup 负责 show，两者与页面脚本谁先谁后
      // 不确定；先按实际可见性对齐一次，避免宠物在没人看见的窗里解码/换动作。
      const syncVisibility = async () => {
        try {
          const visible = await getCurrentWindow().isVisible();
          useNoteStore.getState().setMediaPaused(!visible);
        } catch {
          // 非 Tauri 环境
        }
      };
      await syncVisibility();
      // setup 的 window.show() 可能比页面脚本晚一拍，1 秒后再核对一次，
      // 否则会把刚启动的宠物误判成「藏在托盘里」。
      setTimeout(() => void syncVisibility(), 1000);
      // 恢复形态与窗口几何（window-state 已不再记几何，靠我们自己落位）
      await fetchUiState();
      // 1) Settle previous commission + auto-create today's.
      const daily = await initDaily();
      const petMode = useNoteStore.getState().form === "pet";
      const settlementSpoken = announceSettlement(daily);
      // 2) Load notes, config, balance.
      fetchNotes();
      fetchOpacity();
      fetchSkin();
      fetchTheme();
      getBalance();
      await fetchProgress();
      if (petMode && !settlementSpoken) {
        const p = useNoteStore.getState().progress;
        // 提醒的是「今天填了没」：学习/工作、生活/其他 两个模块都是空白占位才开口
        if (p?.note_found && !p.tomorrow_filled) {
          void useNoteStore.getState().say("主人，「学习/工作」和「生活/其他」还空着哦～", 10_000);
        }
      }
      // 用户从不移动窗口时 onMoved 不会触发；启动落位后补存一次，保证下次回到同一位置
      setTimeout(() => void useNoteStore.getState().autosaveGeometry(), 1000);
    };
    boot();
  }, []);

  // 窗口可能连着开好几天不关：跨过午夜后要自己结算昨天、创建今天。
  // 定时器在后台会被节流，所以窗口恢复焦点时也查一次。
  useEffect(() => {
    let lastDay = localDateKey();
    let ticking = false;
    const checkDay = async () => {
      const today = localDateKey();
      if (today === lastDay || ticking) return;
      const previousDay = lastDay;
      lastDay = today;
      // 用户正在便签里输入（有未落盘改动）时先别结算：否则会按 500ms 前的旧内容算奖励。
      // 把日期退回去，等保存完成后的下一拍再试。
      const before = useNoteStore.getState();
      if (before.form === "note" && before.hasPendingEdit) {
        lastDay = previousDay;
        return;
      }
      ticking = true;
      try {
        const daily = await useNoteStore.getState().initDaily();
        // initDaily 内部把 invoke 失败吞成 null；跨日巡检要把它当成失败，下个 tick 重试
        if (!daily) throw new Error("initDaily 返回空");
        announceSettlement(daily);
        await useNoteStore.getState().fetchNotes();
        await useNoteStore.getState().fetchProgress();

        const st = useNoteStore.getState();
        const todayNote = st.notes.find((n) => n.created_at.startsWith(today));
        const active = st.notes.find((n) => n.id === st.activeId);
        // 宠物态的编辑器是隐藏的，DOM 里可能还留着旧的焦点，不能因此不切换
        const editing =
          st.form === "note" &&
          !!(document.activeElement as HTMLElement | null)?.closest?.(".ProseMirror");
        // 只把“昨天那张每日委托”换成今天；用户在便签态翻旧便签/正在输入时不要抢走。
        // 宠物态没有可见输入，即使有保存正在飞也可以切走（NoteEditor 会先落盘旧便签）。
        const canSwitch = st.form === "pet" || (!st.hasPendingEdit && !editing);
        if (todayNote && (!active || active.created_at.startsWith(previousDay)) && canSwitch) {
          st.setActive(todayNote.id);
        }
      } catch (e) {
        // 初始化失败（磁盘暂时被占用等）时把日期退回去，下一个 tick 再试
        console.error("跨日刷新失败:", e);
        lastDay = previousDay;
      } finally {
        ticking = false;
      }
    };
    const timer = setInterval(() => void checkDay(), 30_000);
    const onFocus = () => void checkDay();
    window.addEventListener("focus", onFocus);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
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

  // 窗口被隐藏/显示（托盘、全屏避让）时暂停或恢复解码。
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

  // save_note 比较出「这次保存多勾掉了几条委托」就广播 task-completed。
  // 这里只把账记进 store：勾复选框时便签态根本没有宠物组件，金袋要等宠物下次
  // 出现在屏幕上（缩回 / 从隐藏中恢复）才兑现。
  useEffect(() => {
    let un: (() => void) | undefined;
    let dead = false;
    listen("task-completed", () => {
      useNoteStore.setState({ coinsPending: true });
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
  // 用 pointerleave/pointerenter 触发，不轮询光标位置（§5.2 的 Win10 黑屏诱因）——
  // 只在事件已经排定的那一次向 OS 求证。
  useEffect(() => {
    // 设置/更新/周结算弹窗都是便签态 UI：它们开着时缩成宠物会瞬间藏起面板，
    // 紧接着又被上面的“弹窗打开就展开”effect 拉回来，形成闪跳。
    if (!petEnabled || form !== "note") return;
    if (settingsOpen || updateState || weeklyResult) return;
    const autoMs = petAutoCollapseMs;
    if (autoMs <= 0) return;
    let timer: ReturnType<typeof setTimeout> | null = null;
    // gen 用来作废在途判定：等 OS 回话的这几毫秒里指针可能已经回到窗口了
    let gen = 0;
    let probes = 0;
    const cancel = () => {
      gen += 1;
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
    };
    const tryCollapse = async (ticket: number) => {
      if (ticket !== gen) return;
      timer = null;
      // 到点先问 OS 两件事：左键还按着 = 用户正拿着这扇窗（拖动与拉边框都走系统模态循环，
      // 那期间 WebView 的 pointerdown / buttons / pointerup 一个都收不到，e.buttons 挡不住）；
      // 光标还在外框矩形里 = 只是把手伸到那圈隐形的可拉伸边框上准备改大小，人没离开便签。
      const holding = (await isMouseButtonDown()) || (await isCursorOnWindow());
      if (ticket !== gen) return;
      if (holding) {
        // 重排而不是就地拒绝：拒完没有下一拍，便签就再也收不回去了（§13-21 那类事故）。
        // 但光标长期停在边框上也不该每半秒空转，问够次数就作罢——宁可便签一直开着。
        if (++probes >= COLLAPSE_PROBE_MAX) return;
        timer = setTimeout(() => void tryCollapse(ticket), autoMs);
        return;
      }
      probes = 0;
      await useNoteStore.getState().setForm("pet");
    };
    const onLeave = (e: PointerEvent) => {
      // 按住键离开 = 正在拖这扇窗：此刻排缩回会在拖拽中途把窗缩成宠物盒，setSize 还可能
      // 被拖拽循环吃掉，结果是「和便签一样大的宠物」。等下一次真正空手移出再收。
      if (e.buttons !== 0) return;
      cancel();
      probes = 0;
      const ticket = gen;
      timer = setTimeout(() => void tryCollapse(ticket), autoMs);
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
  }, [petEnabled, form, petAutoCollapseMs, settingsOpen, updateState, weeklyResult]);

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
