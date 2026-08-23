import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { NoteMeta, InitDailyResult, WeeklyResult, PinMode, AppTheme } from "../types";

const APP_THEMES = ["light", "sky", "sakura", "green"] as const;

function isAppTheme(value: string): value is AppTheme {
  return (APP_THEMES as readonly string[]).includes(value);
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
    // Clear any pending hide timer from a previous cycle
    if (alarmTimeoutId !== null) {
      clearTimeout(alarmTimeoutId);
      alarmTimeoutId = null;
    }
    try {
      const win = getCurrentWindow();
      await win.show();
      await win.setAlwaysOnTop(true);
    } catch {
      return; // 窗口操作失败，本轮放弃
    }
    // 聚焦尽力而为，失败不影响后续隐藏流程
    getCurrentWindow().setFocus().catch(() => {});
    alarmTimeoutId = setTimeout(async () => {
      alarmTimeoutId = null;
      try {
        const win = getCurrentWindow();
        await win.setAlwaysOnTop(false);
        // 结束行为跟随“关闭行为”设置：托盘→隐藏，直接退出→最小化到任务栏
        const toTray = useNoteStore.getState().closeToTray ?? true;
        if (toTray) {
          await win.hide();
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
}

export const useNoteStore = create<NoteState>((set, get) => ({
  notes: [],
  activeId: null,
  pinMode: "normal",
  opacity: 0.92,
  closeToTray: null,
  yuanShi: 0,
  streak: 0,
  alarmInterval: 10, // 分钟
  alarmDuration: 15,
  skinImage: "",
  skinActive: false,
  theme: "light",

  fetchNotes: async () => {
    try {
      const notes: NoteMeta[] = await invoke("list_notes");
      set({ notes });
      if (notes.length > 0 && !get().activeId) {
        // 默认打开当天的委托便签（created_at 为 YYYY-MM-DD 前缀）
        const now = new Date();
        const today =
          `${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}`;
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
      set({ yuanShi: result.yuan_shi });
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
        get().activeId === id
          ? notes.length > 0
            ? notes[0].id
            : null
          : get().activeId;
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
    const next: PinMode =
      pinMode === "pin" ? "alarm" : pinMode === "alarm" ? "normal" : "pin";
    await get().setPinMode(next);
  },

  setPinMode: async (mode: PinMode) => {
    const { alarmInterval, alarmDuration } = get();
    const win = getCurrentWindow();

    stopAlarmTimer();

    if (mode === "pin") {
      await win.setAlwaysOnTop(true);
    } else if (mode === "alarm") {
      await win.setAlwaysOnTop(false);
      startAlarmTimer(alarmInterval * 60 * 1000, alarmDuration * 1000);
    } else {
      // normal
      await win.setAlwaysOnTop(false);
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
      const streak: number = await invoke("get_streak");
      set({ yuanShi, streak });
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