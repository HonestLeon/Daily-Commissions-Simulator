import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { NoteMeta } from "../types";

interface NoteState {
  notes: NoteMeta[];
  activeId: string | null;
  isPinned: boolean;
  opacity: number;

  fetchNotes: () => Promise<void>;
  createNote: () => Promise<string | null>;
  deleteNote: (id: string) => Promise<void>;
  renameNote: (id: string, title: string) => Promise<void>;
  setActive: (id: string) => void;
  togglePin: () => void;
  setOpacity: (value: number) => void;
}

export const useNoteStore = create<NoteState>((set, get) => ({
  notes: [],
  activeId: null,
  isPinned: false,
  opacity: 0.92,

  fetchNotes: async () => {
    try {
      const notes: NoteMeta[] = await invoke("list_notes");
      set({ notes });
      // Auto-select first note if none active
      if (notes.length > 0 && !get().activeId) {
        set({ activeId: notes[0].id });
      }
    } catch (e) {
      console.error("Failed to fetch notes:", e);
    }
  },

  createNote: async () => {
    try {
      const note: NoteMeta = await invoke("create_note", {
        title: "无标题便签",
      });
      const notes = [...get().notes, note];
      set({ notes, activeId: note.id });
      return note.id;
    } catch (e) {
      console.error("Failed to create note:", e);
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
        notes: get().notes.map((n) =>
          n.id === id ? { ...n, title } : n
        ),
      });
    } catch (e) {
      console.error("Failed to rename note:", e);
    }
  },

  setActive: (id: string) => set({ activeId: id }),

  togglePin: () => set({ isPinned: !get().isPinned }),

  setOpacity: (value: number) => set({ opacity: value }),
}));
