import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { NoteMeta, InitDailyResult, WeeklyResult, PinMode } from "../types";

// Module-level timer IDs for alarm mode
let alarmIntervalId: ReturnType<typeof setInterval> | null = null;
let alarmTimeoutId: ReturnType<typeof setTimeout> | null = null;

function stopAlarmTimer() {
  if (alarmIntervalId !== null) {
    clearInterval(alarmIntervalId);
    alarmIntervalId = null;
  }
  if (alarmTimeoutId !== null) {
    clearTimeout(alarmTimeoutId);
    alarmTimeoutId = null;
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
      await win.setFocus();
      alarmTimeoutId = setTimeout(async () => {
        try {
          await win.setAlwaysOnTop(false);
          await win.hide();
        } catch {
          // window may already be closed
        }
      }, durationMs);
    } catch {
      // ignore errors during alarm cycle
    }
  };
  // First cycle immediately
  runCycle();
  alarmIntervalId = setInterval(runCycle, intervalMs);
}

interface NoteState {
  notes: NoteMeta[];
  activeId: string | null;
  pinMode: PinMode;
  opacity: number;
  darkMode: boolean;
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
  fetchDarkMode: () => Promise<void>;
  setDarkMode: (dark: boolean) => Promise<void>;
  fetchCloseBehavior: () => Promise<void>;
  setCloseBehavior: (closeToTray: boolean) => Promise<void>;
  checkWeekly: () => Promise<WeeklyResult>;
  getBalance: () => Promise<void>;
  fetchAlarmConfig: () => Promise<void>;
  setAlarmInterval: (seconds: number) => Promise<void>;
  setAlarmDuration: (seconds: number) => Promise<void>;
}

export const useNoteStore = create<NoteState>((set, get) => ({
  notes: [],
  activeId: null,
  pinMode: "normal",
  opacity: 0.92,
  darkMode: false,
  closeToTray: null,
  yuanShi: 0,
  streak: 0,
  alarmInterval: 10, // 分钟
  alarmDuration: 15,

  fetchNotes: async () => {
    try {
      const notes: NoteMeta[] = await invoke("list_notes");
      set({ notes });
      if (notes.length > 0 && !get().activeId) {
        const lastId = localStorage.getItem("last-active-note");
        const restored = lastId && notes.some((n) => n.id === lastId);
        set({ activeId: restored ? lastId : notes[0].id });
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
    localStorage.setItem("last-active-note", id);
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

  fetchDarkMode: async () => {
    try {
      const dark: boolean = await invoke("get_dark_mode");
      set({ darkMode: dark });
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    } catch {
      // keep default light
    }
  },

  setDarkMode: async (dark: boolean) => {
    try {
      await invoke("set_dark_mode", { darkMode: dark });
      set({ darkMode: dark });
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    } catch (e) {
      console.error("Failed to set dark mode:", e);
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
    set({ streak: result.streak, yuanShi: get().yuanShi + result.reward });
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
      // 后端以秒存储，前端以分钟展示
      const alarmIntervalSec: number = await invoke("get_alarm_interval");
      const alarmDuration: number = await invoke("get_alarm_duration");
      set({ alarmInterval: Math.round(alarmIntervalSec / 60), alarmDuration });
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
}));