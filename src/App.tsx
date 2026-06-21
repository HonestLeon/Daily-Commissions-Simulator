import { useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import TitleBar from "./components/TitleBar";
import Sidebar from "./components/Sidebar";
import NoteEditor from "./components/NoteEditor";
import { useNoteStore } from "./stores/noteStore";

function App() {
  const { fetchNotes, createNote, opacity } = useNoteStore();
  const initialized = useRef(false);

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;

    // Load notes on mount
    fetchNotes();

    // Listen for tray "new note" event
    const unlisten = listen("tray-new-note", () => {
      createNote();
    });

    // Restore pin state
    getCurrentWindow().isAlwaysOnTop().then((pinned) => {
      useNoteStore.setState({ isPinned: pinned });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div className="app" style={{ opacity }}>
      <TitleBar />
      <div className="app-body">
        <Sidebar />
        <main className="editor-area">
          <NoteEditor />
        </main>
      </div>
    </div>
  );
}

export default App;
