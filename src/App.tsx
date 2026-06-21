import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import TitleBar from "./components/TitleBar";
import Sidebar from "./components/Sidebar";
import NoteEditor from "./components/NoteEditor";
import SettingsPanel from "./components/SettingsPanel";
import { useNoteStore } from "./stores/noteStore";

function App() {
  const { fetchNotes, createNote, opacity } = useNoteStore();
  const initialized = useRef(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [focused, setFocused] = useState(true);

  // User-set opacity when focused (editing), fully opaque when not focused (to see through)
  const effectiveOpacity = focused ? opacity : 1.0;

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;

    fetchNotes();

    const unlistenTray = listen("tray-new-note", () => {
      createNote();
    });

    const win = getCurrentWindow();
    win.isAlwaysOnTop().then((pinned) => {
      useNoteStore.setState({ isPinned: pinned });
    });

    // Use DOM focus/blur events (browser-native, works reliably in WebView)
    const onFocus = () => setFocused(true);
    const onBlur = () => setFocused(false);

    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);

    // Check initial state
    setFocused(document.hasFocus());

    return () => {
      unlistenTray.then((fn) => fn());
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  return (
    <div
      className="app"
      style={{
        opacity: effectiveOpacity,
        transition: "opacity 0.3s ease",
      }}
    >
      <TitleBar onSettings={() => setSettingsOpen(true)} />
      <div className="app-body">
        <Sidebar />
        <main className="editor-area">
          <NoteEditor />
        </main>
      </div>
      <SettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}

export default App;
