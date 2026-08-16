import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { NoteMeta, InitDailyResult, WeeklyResult } from "../types";

interface NoteState {
  notes: NoteMeta[];
  activeId: string | null;
  isPinned: boolean;
  opacity: number;
  darkMode: boolean;
  closeToTray: boolean | null;  // null = first time (not set yet)
  yuanShi: number;
  streak: number;

  fetchNotes: () => Promise<void>;
  initDaily: () => Promise<InitDailyResult | null>;
  deleteNote: (id: string) => Promise<void>;
  renameNote: (id: string, title: string) => Promise<void>;
  setActive: (id: string) => void;
  togglePin: () => void;
  setOpacity: (value: number) => void;
  fetchOpacity: () => Promise<void>;
  fetchDarkMode: () => Promise<void>;
  setDarkMode: (dark: boolean) => Promise<void>;
  fetchCloseBehavior: () => Promise<void>;
  setCloseBehavior: (closeToTray: boolean) => Promise<void>;
  checkWeekly: () => Promise<WeeklyResult>;
  getBalance: () => Promise<void>;
}

export const useNoteStore = create<NoteState>((set, get) => ({
  notes: [],
  activeId: null,
  isPinned: false,
  opacity: 0.92,
  darkMode: false,
  closeToTray: null,
  yuanShi: 0,
  streak: 0,

  fetchNotes: async () => {
    try {
      const notes: NoteMeta[] = await invoke("list_notes");
      set({ notes });
      if (notes.length > 0 && !get().activeId) {
        // Restore last active note, or fallback to the most recent.
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

  togglePin: () => set({ isPinned: !get().isPinned }),

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
      set({ closeToTray: true }); // default
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
}));
