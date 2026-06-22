import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { NoteMeta, TemplateMeta } from "../types";

interface NoteState {
  notes: NoteMeta[];
  activeId: string | null;
  isPinned: boolean;
  opacity: number;
  templates: TemplateMeta[];
  templatePickerOpen: boolean;

  fetchNotes: () => Promise<void>;
  createNote: (templateId?: string) => Promise<string | null>;
  deleteNote: (id: string) => Promise<void>;
  renameNote: (id: string, title: string) => Promise<void>;
  setActive: (id: string) => void;
  togglePin: () => void;
  setOpacity: (value: number) => void;
  fetchTemplates: () => Promise<void>;
  saveAsTemplate: (name: string, content: string) => Promise<void>;
  deleteTemplate: (id: string) => Promise<void>;
  setTemplatePickerOpen: (open: boolean) => void;
}

export const useNoteStore = create<NoteState>((set, get) => ({
  notes: [],
  activeId: null,
  isPinned: false,
  opacity: 0.92,
  templates: [],
  templatePickerOpen: false,

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

  createNote: async (templateId?: string) => {
    try {
      let note: NoteMeta;
      if (templateId) {
        note = await invoke("create_note_from_template", { templateId });
      } else {
        note = await invoke("create_note", { title: "无标题便签" });
      }
      const notes = [...get().notes, note];
      set({ notes, activeId: note.id, templatePickerOpen: false });
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

  fetchTemplates: async () => {
    try {
      const templates: TemplateMeta[] = await invoke("list_templates");
      set({ templates });
    } catch (e) {
      console.error("Failed to fetch templates:", e);
    }
  },

  saveAsTemplate: async (name: string, content: string) => {
    try {
      const meta = await invoke<TemplateMeta>("save_template", { name, content });
      set({ templates: [...get().templates, meta] });
    } catch (e) {
      console.error("Failed to save template:", e);
    }
  },

  deleteTemplate: async (id: string) => {
    try {
      await invoke("delete_template", { id });
      set({ templates: get().templates.filter((t) => t.id !== id) });
    } catch (e) {
      console.error("Failed to delete template:", e);
    }
  },

  setTemplatePickerOpen: (open: boolean) => set({ templatePickerOpen: open }),
}));
