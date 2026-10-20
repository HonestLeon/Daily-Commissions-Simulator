import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import {
  getCurrentWindow,
  currentMonitor,
  LogicalSize,
  PhysicalPosition,
  PhysicalSize,
} from "@tauri-apps/api/window";
import type {
  NoteMeta,
  InitDailyResult,
  WeeklyResult,
  PinMode,
  AppTheme,
  Form,
  UiState,
  PetConfig,
  TodayProgress,
  BubblePayload,
} from "../types";

const APP_THEMES = ["light", "sky", "sakura", "green"] as const;

function isAppTheme(value: string): value is AppTheme {
  return (APP_THEMES as readonly string[]).includes(value);
}

// ── 形态（便签 ⇄ 桌宠）常量 ──

// 便签态的窗口下限必须与 tauri.conf.json 的 minWidth/minHeight 一致：宠物态要靠
// setMinSize(null) 先放开下限才缩得动（set_size 会被下限静默夹住），展开时再装回去。
const NOTE_MIN_W = 340;
const NOTE_MIN_H = 300;
const NOTE_DEFAULT_W = 620;
const NOTE_DEFAULT_H = 500;
// 桌宠态窗口 = 精灵可见包围盒（逻辑像素），宽高比照 α-WebM 素材的 220:278
// （175/221 = 0.7919，素材 220/278 = 0.7914；对不上会让视频层出现微小拉伸或留白）。
const PET_BOX_W = 175;
const PET_BOX_H = 221;
// 缩回时先让 React 把内容换成宠物，再隔一帧改窗口尺寸，避免带着便签布局去 resize
const PET_SWAP_MS = 60;
// 缩回锚点：0=贴便签左下角（宠物留在原便签的左下方），1=贴便签右下角
const PET_ANCHOR_RIGHT = false;

// ── 空闲门控与全屏避让（PR7）──

// 只在「主人闲下来 8 秒 ~ 离开 10 分钟」这段窗口期开口：8 秒内说话是打扰，超过 10
// 分钟人早走了，说给他也没用（区间取自参考项目的 8–600 s）。
const IDLE_MIN_SEC = 8;
const IDLE_MAX_SEC = 600;
// 主人一直忙时不能把话无限期憋着：有界等待，超时这条就作废
const IDLE_WAIT_MAX_MS = 180_000;
const IDLE_RETRY_MS = 5_000;
// 避让轮询的区间（秒）。这是后台传感器式的检查，不是用户能感知的节律，所以给个
// 区间加随机抖动即可，重点是别让宠物态一直零轮询（躲不及时）或狂轮询（白耗 CPU）。
const AVOID_MIN_SEC = 4;
const AVOID_MAX_SEC = 8;

/** 下一次避让巡检的间隔毫秒 */
export function avoidancePollMs(): number {
  return (AVOID_MIN_SEC + Math.random() * (AVOID_MAX_SEC - AVOID_MIN_SEC)) * 1000;
}

/** 用户多久没动键鼠；读不到（非 Tauri 或命令失败）返回 null，调用方据此「不门控」 */
async function idleSeconds(): Promise<number | null> {
  try {
    return await invoke<number>("get_idle_seconds");
  } catch {
    return null;
  }
}

/** 前台是否有别的应用全屏 */
async function foregroundFullscreen(): Promise<boolean> {
  try {
    return await invoke<boolean>("is_foreground_fullscreen");
  } catch {
    return false; // 读不到就当没有，宁可不躲也别把宠物弄丢
  }
}

// 一条还没吐出来的话会被下一条顶掉（sayGen 单调），延迟队列只保留最新的那条。
let sayGen = 0;
let sayTimer: ReturnType<typeof setTimeout> | null = null;

/** 物理像素下的窗口几何（outerPosition + innerSize） */
interface Geometry {
  x: number;
  y: number;
  w: number;
  h: number;
}

// 形态切换是「一串有先后顺序的窗口调用」，用 store 里的 formBusy 当互斥锁：
// 并发会让 focusable / setMinSize 的成对操作错配，出现「窗口是便签大小但拿不到焦点」
// 这类难查状态；同时也让几何自动保存知道「现在正在切形态，别插手」。

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** 正在编辑器里打字/选字时不许缩回：鼠标一滑就收起会打断输入 */
function editorBusy(): boolean {
  const ae = document.activeElement as HTMLElement | null;
  return !!ae?.closest?.(".ProseMirror");
}

/** 供 CSS 区分形态（与现有 data-theme / data-skin 同一套路） */
function applyFormMarker(form: Form) {
  document.documentElement.dataset.form = form;
}

async function scaleFactorOrDefault(): Promise<number> {
  try {
    return (await getCurrentWindow().scaleFactor()) || 1;
  } catch {
    return 1; // 非 Tauri 环境（纯浏览器预览）
  }
}

/** 当前窗口所在显示器的工作区（物理像素）；拿不到就返回 null 表示不夹取 */
async function workAreaPhysical(): Promise<Geometry | null> {
  try {
    const mon = await currentMonitor();
    if (mon) {
      return {
        x: mon.workArea.position.x,
        y: mon.workArea.position.y,
        w: mon.workArea.size.width,
        h: mon.workArea.size.height,
      };
    }
  } catch {
    // 非 Tauri 环境无显示器信息，按不夹取处理
  }
  return null;
}

/** 读回窗口当前的真实几何，落位后一律以它为准写进配置（不记「打算设成多少」） */
async function readWindowGeometry(): Promise<Geometry> {
  const win = getCurrentWindow();
  const pos = await win.outerPosition().catch(() => null);
  const size = await win.innerSize().catch(() => null);
  return {
    x: pos?.x ?? 0,
    y: pos?.y ?? 0,
    w: size?.width ?? 0,
    h: size?.height ?? 0,
  };
}

/**
 * 只在窗口真的可见时读几何，否则返回 null。Windows 把最小化的窗口挪到 (-32000,-32000)
 * 并塞一个 288×35 的占位尺寸；发布版实测到一次形态切换把这坨垃圾当成便签态几何写进了
 * config.json，下次展开就会落到屏幕外。凡是「读来存档 / 读来定位气泡」的都必须走这里。
 */
async function readVisibleWindowGeometry(): Promise<Geometry | null> {
  try {
    if (await getCurrentWindow().isMinimized()) return null;
  } catch {
    return null; // 非 Tauri 环境
  }
  const g = await readWindowGeometry();
  return g.w > 0 && g.h > 0 ? g : null;
}

/** 把窗口左上角夹进工作区（窗口比工作区还大时退化为贴左上） */ function clampToWorkArea(
  wa: Geometry | null,
  x: number,
  y: number,
  w: number,
  h: number
): { x: number; y: number } {
  if (!wa) return { x, y };
  const maxX = Math.max(wa.x, wa.x + wa.w - w);
  const maxY = Math.max(wa.y, wa.y + wa.h - h);
  return {
    x: Math.min(Math.max(x, wa.x), maxX),
    y: Math.min(Math.max(y, wa.y), maxY),
  };
}

/**
 * 距工作区边缘 14 px 以内就吸附贴边（照抄参考项目 CompanionEdgeLayout.Clamp 的手感）。
 * 只给宠物态用：便签态是正经工作窗口，不该被偷偷挪位置。
 */
function snapToWorkArea(
  wa: Geometry | null,
  x: number,
  y: number,
  w: number,
  h: number
): { x: number; y: number } {
  const clamped = clampToWorkArea(wa, x, y, w, h);
  if (!wa) return clamped;
  const EPS = 14;
  let { x: nx, y: ny } = clamped;
  if (Math.abs(nx - wa.x) < EPS) nx = wa.x;
  if (Math.abs(nx + w - (wa.x + wa.w)) < EPS) nx = wa.x + wa.w - w;
  if (Math.abs(ny - wa.y) < EPS) ny = wa.y;
  if (Math.abs(ny + h - (wa.y + wa.h)) < EPS) ny = wa.y + wa.h - h;
  return { x: nx, y: ny };
}

/**
 * 展开成便签态。顺序不能反：先把焦点能力与尺寸下限装回去，再落位，最后才由调用方
 * 让 React 渲染便签内容——反过来会出现「175×220 的窗里画一整套便签布局」的闪烁。
 */
async function expandToNoteForm(geom: Geometry | null, pinMode: PinMode) {
  const win = getCurrentWindow();
  await win.setFocusable(true).catch(() => {});
  // 便签要能拉大，也要那圈投影；宠物反过来
  await win.setResizable(true).catch(() => {});
  await win.setShadow(true).catch(() => {});
  await win.setMinSize(new LogicalSize(NOTE_MIN_W, NOTE_MIN_H)).catch(() => {});
  // 存档几何是物理像素，下限是逻辑像素：统一到逻辑做取舍，再乘回物理
  const sf = await scaleFactorOrDefault();
  const logW = geom ? geom.w / sf : NOTE_DEFAULT_W;
  const logH = geom ? geom.h / sf : NOTE_DEFAULT_H;
  const w = Math.round(Math.max(NOTE_MIN_W, logW) * sf);
  const h = Math.round(Math.max(NOTE_MIN_H, logH) * sf);
  const wa = await workAreaPhysical();
  const pos = clampToWorkArea(wa, geom?.x ?? 0, geom?.y ?? 0, w, h);
  await win.setSize(new PhysicalSize(w, h)).catch(() => {});
  await win.setPosition(new PhysicalPosition(pos.x, pos.y)).catch(() => {});
  await win.setAlwaysOnTop(pinMode === "pin").catch(() => {});
}

/**
 * 缩回宠物态。宠物恒置顶、恒不吃焦点：点击/显示宠物绝不能把焦点从用户正在打字的地方抢走。
 * 锚点：有存档位置就用存档（用户可能把它拖到别处了），否则贴着便签窗的下沿一角。
 */
async function collapseToPetForm(petPos: { x: number; y: number } | null, scale: number) {
  const win = getCurrentWindow();
  const cur = await readWindowGeometry();
  // 不先放开下限就会被夹在 340×300，缩不动还看起来像「没反应」
  await win.setMinSize(null).catch(() => {});
  // 宠物不需要拉伸：去掉 resizable 就是去掉 WS_THICKFRAME —— 那圈看不见的可拖边框
  // （200% 下实测 26×15 物理像素）既白吃桌面点击，又让窗口比包围盒大一圈；
  // 投影一并关掉，透明窗四周就不会显出一层灰边。必须在 setSize 之前做，
  // 否则边框没了之后客户区尺寸会重算一次，量出来的尺寸又不是我们要的。
  await win.setResizable(false).catch(() => {});
  await win.setShadow(false).catch(() => {});
  const sf = await scaleFactorOrDefault();
  const w = Math.round(PET_BOX_W * scale * sf);
  const h = Math.round(PET_BOX_H * scale * sf);
  const x = petPos ? petPos.x : PET_ANCHOR_RIGHT ? cur.x + cur.w - w : cur.x;
  const y = petPos ? petPos.y : cur.y + cur.h - h;
  const wa = await workAreaPhysical();
  const pos = snapToWorkArea(wa, x, y, w, h);
  await win.setSize(new PhysicalSize(w, h)).catch(() => {});
  await win.setPosition(new PhysicalPosition(pos.x, pos.y)).catch(() => {});
  await win.setAlwaysOnTop(true).catch(() => {});
  // 焦点最后放掉：中途失去可聚焦会把 pointerup 一起吞掉
  await win.setFocusable(false).catch(() => {});
}

/** 把形态与该形态的几何写回 Rust 配置（宠物态只存位置，尺寸由 pet_scale 推导） */
async function persistFormGeometry(form: Form, geom: Geometry) {
  try {
    await invoke("save_ui_state", {
      form,
      x: geom.x,
      y: geom.y,
      w: form === "note" ? geom.w : null,
      h: form === "note" ? geom.h : null,
    });
  } catch (e) {
    console.error("Failed to persist form:", e);
  }
}

// Module-level timer IDs for alarm mode
let alarmIntervalId: ReturnType<typeof setInterval> | null = null;
let alarmTimeoutId: ReturnType<typeof setTimeout> | null = null;
let alarmFirstId: ReturnType<typeof setTimeout> | null = null;

function stopAlarmTimer() {
  if (alarmIntervalId !== null) {
    clearInterval(alarmIntervalId);
    alarmIntervalId = null;
  }
  if (alarmTimeoutId !== null) {
    clearTimeout(alarmTimeoutId);
    alarmTimeoutId = null;
  }
  if (alarmFirstId !== null) {
    clearTimeout(alarmFirstId);
    alarmFirstId = null;
  }
}

function startAlarmTimer(intervalMs: number, durationMs: number) {
  stopAlarmTimer();
  const runCycle = async () => {
    // 宠物态下的「置顶提醒」= 把便签弹出来给用户看，而不是把小窗反复置顶
    if (useNoteStore.getState().form === "pet") {
      await useNoteStore.getState().setForm("note");
    }
    // Clear any pending hide timer from a previous cycle
    if (alarmTimeoutId !== null) {
      clearTimeout(alarmTimeoutId);
      alarmTimeoutId = null;
    }
    try {
      const win = getCurrentWindow();
      // 用户可能先把窗最小化了：show() 解不开最小化，闹钟会「响了但什么都没出现」
      await win.unminimize();
      await win.show();
      await win.setAlwaysOnTop(true);
      // 从这一行起画面重新可见，桌宠若在隐藏中攒下的暂停要放开
      useNoteStore.getState().setMediaPaused(false);
    } catch {
      return; // 窗口操作失败，本轮放弃
    }
    // 聚焦尽力而为，失败不影响后续隐藏流程
    getCurrentWindow()
      .setFocus()
      .catch(() => {});
    alarmTimeoutId = setTimeout(async () => {
      alarmTimeoutId = null;
      try {
        const win = getCurrentWindow();
        await win.setAlwaysOnTop(false);
        // 结束行为跟随“关闭行为”设置：托盘→隐藏，直接退出→最小化到任务栏
        const toTray = useNoteStore.getState().closeToTray ?? true;
        if (toTray) {
          await win.hide();
          useNoteStore.getState().setMediaPaused(true);
        } else {
          await win.minimize();
        }
      } catch {
        // window may already be closed
      }
    }, durationMs);
  };
  // 首轮 1 分钟后触发，之后按设置间隔循环（间隔最小 1 分钟）
  alarmFirstId = setTimeout(() => {
    runCycle();
    alarmIntervalId = setInterval(runCycle, Math.max(60_000, intervalMs));
  }, 60_000);
}

interface NoteState {
  notes: NoteMeta[];
  activeId: string | null;
  pinMode: PinMode;
  opacity: number;
  closeToTray: boolean | null;
  yuanShi: number;
  mora: number;
  streak: number;
  alarmInterval: number;
  alarmDuration: number;

  fetchNotes: () => Promise<void>;
  initDaily: () => Promise<InitDailyResult | null>;
  deleteNote: (id: string) => Promise<void>;
  renameNote: (id: string, title: string) => Promise<void>;
  setActive: (id: string) => void;
  cyclePinMode: () => Promise<void>;
  setPinMode: (mode: PinMode) => Promise<void>;
  setOpacity: (value: number) => void;
  fetchOpacity: () => Promise<void>;
  fetchCloseBehavior: () => Promise<void>;
  setCloseBehavior: (closeToTray: boolean) => Promise<void>;
  checkWeekly: () => Promise<WeeklyResult>;
  getBalance: () => Promise<void>;
  fetchAlarmConfig: () => Promise<void>;
  setAlarmInterval: (minutes: number) => Promise<void>;
  setAlarmDuration: (seconds: number) => Promise<void>;
  skinImage: string;
  skinActive: boolean;
  fetchSkin: () => Promise<void>;
  importSkin: (dataUrl: string) => Promise<void>;
  removeSkin: () => Promise<void>;
  theme: AppTheme;
  fetchTheme: () => Promise<void>;
  setTheme: (theme: AppTheme) => Promise<void>;

  // ── 桌宠形态 ──
  /** 当前窗口形态；petEnabled=false 时恒为 note */
  form: Form;
  /** 桌宠模式总开关：关闭后应用行为与桌宠化之前完全一致 */
  petEnabled: boolean;
  petScale: number;
  /** 动作间隔档位下标（写回设置时用，上下限反推不出是第几档） */
  petActionIndex: number;
  /** 动作间隔区间（秒），区间内随机而非固定周期 */
  petActionMinSec: number;
  petActionMaxSec: number;
  /** 鼠标移出后自动缩回的宽限毫秒；0 = 不自动缩回 */
  petAutoCollapseMs: number;
  petHideOnFullscreen: boolean;
  noteGeom: Geometry | null;
  petPos: { x: number; y: number } | null;
  /** 编辑器有尚未落盘的改动（500ms debounce 窗口内），此时禁止缩回 */
  hasPendingEdit: boolean;
  /** 正在执行形态切换那一串窗口调用，几何自动保存此时不插手 */
  formBusy: boolean;
  /** 当天委托的完成情况（Rust 权威，桌宠的动作与气泡据此挑选） */
  progress: TodayProgress | null;
  /** 最近一次结算发生的时刻（毫秒）；只在几分钟内让宠物有「刚拿到钱」的动作 */
  settledAt: number | null;
  /**
   * 画面不可见 → 暂停解码。窗口隐藏的路径有好几条（托盘、闹钟收尾、避让），
   * 由这些路径各自置位；桌宠在隐藏时继续解码就是白烧 CPU。
   */
  mediaPaused: boolean;
  /** 因为「别人全屏」而主动避让（隐藏）中 */
  petAvoided: boolean;
  fetchUiState: () => Promise<void>;
  setForm: (next: Form) => Promise<void>;
  /** 记录窗口可见性变化（Rust 侧 hide/show 会广播 pet-visibility-changed） */
  setMediaPaused: (paused: boolean) => void;
  /** 他人全屏时整只宠物让路：隐藏 + 收起气泡；退出全屏再回到原位 */
  setPetAvoided: (avoided: boolean) => Promise<void>;
  /** 避让巡检的一步（由 App 的定时器调用）：全屏就躲，不全屏就回来 */
  checkFullscreenAvoidance: () => Promise<void>;
  /** 设置面板「桌宠」分区写回；以 Rust 规范化后的返回值为准 */
  setPetConfig: (patch: PetConfig) => Promise<void>;
  /** 用户拖动/缩放窗口后记录当前形态的几何（window-state 已不再记 SIZE/POSITION） */
  autosaveGeometry: () => Promise<void>;
  fetchProgress: () => Promise<void>;
  /** 让宠物说一句话：文本与锚点广播给 bubble 窗，由那边逐字显示 */
  say: (text: string, ttlMs?: number) => Promise<void>;
}

export const useNoteStore = create<NoteState>((set, get) => ({
  notes: [],
  activeId: null,
  pinMode: "normal",
  opacity: 0.92,
  closeToTray: null,
  yuanShi: 0,
  mora: 0,
  streak: 0,
  alarmInterval: 10, // 分钟
  alarmDuration: 15,
  skinImage: "",
  skinActive: false,
  theme: "light",
  form: "note",
  petEnabled: false,
  petScale: 1,
  petActionIndex: 2,
  petActionMinSec: 45,
  petActionMaxSec: 90,
  petAutoCollapseMs: 500,
  petHideOnFullscreen: true,
  noteGeom: null,
  petPos: null,
  hasPendingEdit: false,
  formBusy: false,
  progress: null,
  settledAt: null,
  mediaPaused: false,
  petAvoided: false,

  fetchNotes: async () => {
    try {
      const notes: NoteMeta[] = await invoke("list_notes");
      set({ notes });
      if (notes.length > 0 && !get().activeId) {
        // 默认打开当天的委托便签（created_at 为 YYYY-MM-DD 前缀）
        const now = new Date();
        const today = `${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}`;
        const todayNote = notes.find((n) => n.created_at.startsWith(today));
        set({ activeId: todayNote ? todayNote.id : notes[0].id });
      }
    } catch (e) {
      console.error("Failed to fetch notes:", e);
    }
  },

  initDaily: async () => {
    try {
      const result: InitDailyResult = await invoke("init_daily");
      set({ yuanShi: result.yuan_shi, mora: result.mora });
      // 记下「刚结算过」，桌宠三分钟内会播金袋/庆祝动作（settled 门控）
      if (result.settle_reward > 0 || result.settle_mora > 0) set({ settledAt: Date.now() });
      return result;
    } catch (e) {
      console.error("Failed to init daily:", e);
      return null;
    }
  },

  deleteNote: async (id: string) => {
    try {
      await invoke("delete_note", { id });
      const notes = get().notes.filter((n) => n.id !== id);
      const activeId =
        get().activeId === id ? (notes.length > 0 ? notes[0].id : null) : get().activeId;
      set({ notes, activeId });
    } catch (e) {
      console.error("Failed to delete note:", e);
    }
  },

  renameNote: async (id: string, title: string) => {
    try {
      await invoke("rename_note", { id, newTitle: title });
      set({
        notes: get().notes.map((n) => (n.id === id ? { ...n, title } : n)),
      });
    } catch (e) {
      console.error("Failed to rename note:", e);
    }
  },

  setActive: (id: string) => {
    set({ activeId: id });
  },

  cyclePinMode: async () => {
    const { pinMode } = get();
    // pin → alarm → normal → pin
    const next: PinMode = pinMode === "pin" ? "alarm" : pinMode === "alarm" ? "normal" : "pin";
    await get().setPinMode(next);
  },

  setPinMode: async (mode: PinMode) => {
    const { alarmInterval, alarmDuration } = get();
    const win = getCurrentWindow();

    stopAlarmTimer();

    if (mode === "pin") {
      await win.setAlwaysOnTop(true);
    } else if (mode === "alarm") {
      // 宠物态恒置顶：桌宠被别的窗口埋掉就不是桌宠了
      if (get().form !== "pet") await win.setAlwaysOnTop(false);
      startAlarmTimer(alarmInterval * 60 * 1000, alarmDuration * 1000);
    } else {
      // normal
      if (get().form !== "pet") await win.setAlwaysOnTop(false);
    }

    set({ pinMode: mode });
  },

  setOpacity: (value: number) => {
    set({ opacity: value });
    invoke("set_opacity", { opacity: value }).catch((e) =>
      console.error("Failed to persist opacity:", e)
    );
  },

  fetchOpacity: async () => {
    try {
      const opacity: number = await invoke("get_opacity");
      set({ opacity });
    } catch {
      // keep default 0.92
    }
  },

  fetchCloseBehavior: async () => {
    try {
      const tray: boolean = await invoke("get_close_behavior");
      set({ closeToTray: tray });
    } catch {
      set({ closeToTray: true });
    }
  },

  setCloseBehavior: async (closeToTray: boolean) => {
    try {
      await invoke("set_close_behavior", { closeToTray });
      set({ closeToTray });
    } catch (e) {
      console.error("Failed to set close behavior:", e);
    }
  },

  checkWeekly: async () => {
    const result: WeeklyResult = await invoke("check_weekly");
    // 原石余额以后端结算后的权威值为准，避免本地累加造成漂移
    set({ streak: result.streak, yuanShi: result.yuan_shi });
    return result;
  },

  getBalance: async () => {
    try {
      const yuanShi: number = await invoke("get_yuan_shi");
      const mora: number = await invoke("get_mora");
      const streak: number = await invoke("get_streak");
      set({ yuanShi, mora, streak });
    } catch (e) {
      console.error("Failed to get balance:", e);
    }
  },

  fetchAlarmConfig: async () => {
    try {
      // 后端以秒存储，前端以分钟展示；至少 1 分钟（旧配置可能读到 0）
      const alarmIntervalSec: number = await invoke("get_alarm_interval");
      const alarmDuration: number = await invoke("get_alarm_duration");
      set({
        alarmInterval: Math.max(1, Math.round(alarmIntervalSec / 60)),
        alarmDuration: Math.max(1, alarmDuration),
      });
    } catch {
      // keep defaults
    }
  },

  setAlarmInterval: async (minutes: number) => {
    const clamped = Math.max(1, Math.round(minutes));
    try {
      await invoke("set_alarm_interval", { seconds: clamped * 60 });
      set({ alarmInterval: clamped });
      // Restart alarm timer if currently in alarm mode
      if (get().pinMode === "alarm") {
        startAlarmTimer(clamped * 60 * 1000, get().alarmDuration * 1000);
      }
    } catch (e) {
      console.error("Failed to set alarm interval:", e);
    }
  },

  setAlarmDuration: async (seconds: number) => {
    const clamped = Math.max(1, Math.min(60, seconds));
    try {
      await invoke("set_alarm_duration", { seconds: clamped });
      set({ alarmDuration: clamped });
      // Restart alarm timer if currently in alarm mode
      if (get().pinMode === "alarm") {
        startAlarmTimer(get().alarmInterval * 60 * 1000, clamped * 1000);
      }
    } catch (e) {
      console.error("Failed to set alarm duration:", e);
    }
  },

  // 皮肤（仅浅色模式生效）：skinImage 为背景 DataURL，skinActive 标记是否启用
  fetchSkin: async () => {
    try {
      const skin = await invoke<string | null>("get_skin");
      set({ skinImage: skin ?? "", skinActive: !!skin });
      applySkinMarker(!!skin);
    } catch (e) {
      console.error("Failed to fetch skin:", e);
    }
  },

  importSkin: async (dataUrl: string) => {
    const skin = await invoke<string>("save_skin", { dataUrl });
    set({ skinImage: skin, skinActive: true });
    applySkinMarker(true);
  },

  removeSkin: async () => {
    try {
      await invoke("remove_skin");
      set({ skinImage: "", skinActive: false });
      applySkinMarker(false);
    } catch (e) {
      console.error("Failed to remove skin:", e);
    }
  },

  // 颜色主题：浅色底深色字（light / sky / sakura / green）
  fetchTheme: async () => {
    try {
      const theme = await invoke<string>("get_theme");
      const valid = isAppTheme(theme) ? theme : "light";
      set({ theme: valid });
      applyThemeMarker(valid);
    } catch (e) {
      console.error("Failed to fetch theme:", e);
    }
  },

  setTheme: async (theme: AppTheme) => {
    try {
      await invoke("set_theme", { theme });
      set({ theme });
      applyThemeMarker(theme);
    } catch (e) {
      console.error("Failed to set theme:", e);
    }
  },

  // ── 桌宠形态 ──

  /** 读回形态/开关/两套几何并按存档落位（window-state 已不再记几何） */
  fetchUiState: async () => {
    let s: UiState;
    try {
      s = await invoke<UiState>("get_ui_state");
    } catch (e) {
      console.error("Failed to fetch ui state:", e);
      return; // 读不到就留在便签态，等价于桌宠化之前的行为
    }
    const petPos = s.pet_x != null && s.pet_y != null ? { x: s.pet_x, y: s.pet_y } : null;
    const noteGeom =
      s.note_w != null && s.note_h != null
        ? { x: s.note_x ?? 0, y: s.note_y ?? 0, w: s.note_w, h: s.note_h }
        : null;
    set({
      petEnabled: s.pet_enabled,
      petScale: s.pet_scale,
      petActionIndex: s.action_index,
      petActionMinSec: s.action_min_sec,
      petActionMaxSec: s.action_max_sec,
      petAutoCollapseMs: s.auto_collapse_ms,
      petHideOnFullscreen: s.hide_on_fullscreen,
      petPos,
      noteGeom,
    });
    // 开关关着 / 存档不明 → 一律便签态；从宠物态被关掉时必须把窗恢复成便签大小
    const prev = get().form;
    const target: Form = s.pet_enabled && s.form === "pet" ? "pet" : "note";
    set({ form: target });
    applyFormMarker(target);
    if (target === "pet") {
      if (prev !== "pet") await sleep(PET_SWAP_MS); // 内容刚换成宠物，隔一帧再缩窗
      await collapseToPetForm(petPos, s.pet_scale);
    } else if (prev !== "note" || noteGeom) {
      await expandToNoteForm(noteGeom, get().pinMode);
    }
  },

  /**
   * 记录当前形态的真实几何。用户手动拖窗/缩放后走这里（window-state 已不再记
   * SIZE/POSITION，不自己存就丢）；宠物态顺手做贴边吸附。
   */
  autosaveGeometry: async () => {
    const st = get();
    if (st.formBusy) return;
    const g = await readVisibleWindowGeometry();
    if (!g) return; // 最小化 / 非 Tauri：此刻的几何是占位垃圾，存了就丢
    let pos = { x: g.x, y: g.y };
    if (st.form === "pet") {
      const wa = await workAreaPhysical();
      pos = snapToWorkArea(wa, g.x, g.y, g.w, g.h);
      if (pos.x !== g.x || pos.y !== g.y) {
        await getCurrentWindow()
          .setPosition(new PhysicalPosition(pos.x, pos.y))
          .catch(() => {});
      }
    }
    const saved = { x: pos.x, y: pos.y, w: g.w, h: g.h };
    set(st.form === "note" ? { noteGeom: saved } : { petPos: { x: pos.x, y: pos.y } });
    await persistFormGeometry(st.form, saved);
  },

  /** 当天三模块完成情况：桌宠的动作门控与气泡文案都用它 */
  fetchProgress: async () => {
    try {
      set({ progress: await invoke<TodayProgress>("get_today_progress") });
    } catch (e) {
      console.error("Failed to fetch progress:", e);
    }
  },

  /**
   * 说一句话。气泡画在独立的 bubble 窗里（不是主窗的浮层）：主窗必须保持等于宠物
   * 包围盒，把气泡塞进来就会在中间留一块点不动的透明死区（§5.1）。
   *
   * 开口前先过两道门：画面不可见（避让/托盘）就别说了；主人正在打字（空闲 < 8 s）
   * 就等一下再说。等待期间若被新的 say 顶掉，旧的直接作废——只讲最新那句。
   */
  say: async (text: string, ttlMs = 8000) => {
    if (!text) return;
    const gen = ++sayGen;
    if (sayTimer) clearTimeout(sayTimer);
    sayTimer = null;

    const send = async () => {
      if (gen !== sayGen) return;
      // 锚点要等到真正开口这一刻才读：等空闲的这几秒里用户可能把宠物拖走了
      const g = await readVisibleWindowGeometry();
      if (!g) return; // 窗口最小化 / 非 Tauri 环境没有气泡窗
      const payload: BubblePayload = { text, ttlMs, anchor: { x: g.x, y: g.y, w: g.w, h: g.h } };
      try {
        await emit("pet-bubble", payload);
      } catch (e) {
        console.error("气泡发送失败:", e);
      }
    };

    const deadline = Date.now() + IDLE_WAIT_MAX_MS;
    const attempt = async () => {
      if (gen !== sayGen) return;
      if (get().mediaPaused) return; // 宠物藏起来了，说了没人听
      const idle = await idleSeconds();
      if (idle === null) return void (await send()); // 读不到就不门控，别把话憋死
      if (idle >= IDLE_MIN_SEC && idle <= IDLE_MAX_SEC) return void (await send());
      if (idle > IDLE_MAX_SEC) return; // 人早走了，这句过期
      if (Date.now() > deadline) return; // 一直在忙：有界等待后放弃
      sayTimer = setTimeout(() => void attempt(), IDLE_RETRY_MS);
    };
    await attempt();
  },

  /** 窗口可见性变化时由 Rust 的 pet-visibility-changed 事件或避让逻辑置位 */
  setMediaPaused: (paused: boolean) => {
    if (get().mediaPaused === paused) return;
    set({ mediaPaused: paused });
  },

  /**
   * 别人全屏（游戏 / 投屏 / 全屏播放）时桌宠整只让路。这是桌宠常驻的口碑线：
   * 挡住游戏比少一只宠物差评得多。退出全屏后回到原位，不改变形态。
   */
  setPetAvoided: async (avoided: boolean) => {
    if (get().petAvoided === avoided) return;
    set({ petAvoided: avoided, mediaPaused: avoided });
    const win = getCurrentWindow();
    if (avoided) {
      // 气泡也要一起收：宠物都不在了，话还挂在空中很怪
      await emit("pet-bubble-hide").catch(() => {});
      await win.hide().catch(() => {});
    } else {
      await win.show().catch(() => {});
    }
  },

  checkFullscreenAvoidance: async () => {
    // 便签态不躲：那时候窗口是用户主动展开在看的东西，不是常驻装饰
    if (get().form !== "pet" || !get().petHideOnFullscreen) {
      if (get().petAvoided) await get().setPetAvoided(false);
      return;
    }
    await get().setPetAvoided(await foregroundFullscreen());
  },

  /**
   * 写回桌宠设置。不在本地乐观赋值：Rust 侧会 normalize（越界静默纠偏），并且它会
   * 广播 ui-state-changed，App 的监听器随后 fetchUiState 把权威值与几何一次落齐。
   */
  setPetConfig: async (patch: PetConfig) => {
    try {
      await invoke("set_pet_config", {
        petEnabled: patch.petEnabled,
        scale: patch.scale,
        actionIndex: patch.actionIndex,
        autoCollapseMs: patch.autoCollapseMs,
        hideOnFullscreen: patch.hideOnFullscreen,
      });
    } catch (e) {
      console.error("Failed to save pet config:", e);
    }
  },

  /** 唯一的形态切换入口：内部串行执行那串有先后顺序的窗口调用 */
  setForm: async (next: Form) => {
    const st = get();
    // 开关关着就永远只有便签态——桌宠化对没启用它的用户必须零感知
    const target: Form = st.petEnabled ? next : "note";
    if (target === st.form) return;
    if (st.formBusy) return; // 并发会让 focusable / setMinSize 成对错配
    if (target === "pet" && (st.hasPendingEdit || editorBusy())) return;

    set({ formBusy: true });
    try {
      if (target === "note") {
        await expandToNoteForm(st.noteGeom, st.pinMode);
        // 展开是用户主动要看的画面：避让与暂停标记到此为止，不能带着「躲起来了」进便签态
        set({ form: "note", petAvoided: false, mediaPaused: false });
        applyFormMarker("note");
      } else {
        // 先换成宠物内容，隔一帧再缩窗；顺序反了会带着便签布局去 resize
        set({ form: "pet" });
        applyFormMarker("pet");
        await sleep(PET_SWAP_MS);
        await collapseToPetForm(st.petPos, st.petScale);
      }
      // 落位后以窗口真实几何为准回写，而不是记「打算设成多少」；
      // 最小化时读到的是 (-32000,-32000)+占位尺寸，宁可不存也不能存它
      const g = await readVisibleWindowGeometry();
      if (g) {
        set(target === "note" ? { noteGeom: g } : { petPos: { x: g.x, y: g.y } });
        await persistFormGeometry(target, g);
      }
    } catch (e) {
      console.error("切换形态失败:", e);
    } finally {
      set({ formBusy: false });
    }
  },
}));

// 供 CSS 区分"是否开启皮肤"（仅浅色模式下的面板透明度由 CSS 使用该标记）
function applySkinMarker(active: boolean) {
  document.documentElement.dataset.skin = active ? "on" : "off";
}

// 供 CSS 按主题覆盖设计令牌（面板底色 / 强调色 / 蒙版色）
function applyThemeMarker(theme: AppTheme) {
  document.documentElement.dataset.theme = theme;
}

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}
