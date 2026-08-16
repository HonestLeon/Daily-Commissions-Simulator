import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import TitleBar from "./components/TitleBar";
import Sidebar from "./components/Sidebar";
import NoteEditor from "./components/NoteEditor";
import SettingsPanel from "./components/SettingsPanel";
import TemplatePicker from "./components/TemplatePicker";
import UpdateDialog, { type UpdateState } from "./components/UpdateDialog";
import { useNoteStore } from "./stores/noteStore";
import type { UpdateInfo } from "./types";

function App() {
  const {
    fetchNotes,
    fetchTemplates,
    fetchCloseBehavior,
    fetchDarkMode,
    fetchOpacity,
    createNote,
    opacity,
  } = useNoteStore();
  const initialized = useRef(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);

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
    const url = updateState.info.download_url;
    setUpdateState({ phase: "installing" });
    try {
      await invoke("download_and_install", { url });
      setUpdateState({ phase: "done" });
    } catch (e) {
      setUpdateState({
        phase: "error",
        message: typeof e === "string" ? e : "更新失败",
      });
    }
  };

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;

    fetchNotes();
    fetchTemplates();
    fetchCloseBehavior();
    fetchDarkMode();
    fetchOpacity();

    const unlisten = listen("tray-new-note", () => {
      createNote();
    });

    getCurrentWindow().isAlwaysOnTop().then((pinned) => {
      useNoteStore.setState({ isPinned: pinned });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Global keyboard shortcuts
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod) return;

      const s = useNoteStore.getState();

      // Ctrl/Cmd + N → new note (open template picker)
      if (!e.shiftKey && e.code === "KeyN") {
        e.preventDefault();
        s.setTemplatePickerOpen(true);
        return;
      }

      // Ctrl/Cmd + D → delete current note
      if (!e.shiftKey && e.code === "KeyD") {
        e.preventDefault();
        if (s.activeId) s.deleteNote(s.activeId);
        return;
      }

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

      // Ctrl/Cmd + Shift + T → toggle always-on-top
      if (e.shiftKey && e.code === "KeyT") {
        e.preventDefault();
        const newPinned = !s.isPinned;
        getCurrentWindow().setAlwaysOnTop(newPinned);
        s.togglePin();
        return;
      }

      // Ctrl/Cmd + Shift + L → toggle dark mode
      if (e.shiftKey && e.code === "KeyL") {
        e.preventDefault();
        s.setDarkMode(!s.darkMode);
        return;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="app" style={{ "--app-bg-opacity": opacity } as React.CSSProperties}>
      <TitleBar
        onSettings={() => setSettingsOpen(true)}
        onCheckUpdate={handleCheckUpdate}
      />
      <div className="app-body">
        <Sidebar />
        <main className="editor-area">
          <NoteEditor />
        </main>
      </div>
      <SettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <UpdateDialog
        state={updateState}
        onClose={() => setUpdateState(null)}
        onUpdate={handleUpdate}
      />
      <TemplatePicker />
    </div>
  );
}

export default App;
