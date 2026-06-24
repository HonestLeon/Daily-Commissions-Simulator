import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import TitleBar from "./components/TitleBar";
import Sidebar from "./components/Sidebar";
import NoteEditor from "./components/NoteEditor";
import SettingsPanel from "./components/SettingsPanel";
import TemplatePicker from "./components/TemplatePicker";
import { useNoteStore } from "./stores/noteStore";

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

  return (
    <div className="app" style={{ "--app-bg-opacity": opacity } as React.CSSProperties}>
      <TitleBar onSettings={() => setSettingsOpen(true)} />
      <div className="app-body">
        <Sidebar />
        <main className="editor-area">
          <NoteEditor />
        </main>
      </div>
      <SettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <TemplatePicker />
    </div>
  );
}

export default App;
