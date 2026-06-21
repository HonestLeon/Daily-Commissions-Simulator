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

  // Fully opaque when focused, user-set opacity when not focused
  const effectiveOpacity = focused ? 1.0 : opacity;

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

    // Listen for focus/blur using raw events
    const setup = async () => {
      const unlistenFocus = await win.listen("tauri://focus", () => {
        console.log("Window focused");
        setFocused(true);
      });
      const unlistenBlur = await win.listen("tauri://blur", () => {
        console.log("Window blurred");
        setFocused(false);
      });

      // Initial state
      win.isFocused().then((f) => setFocused(f));

      return () => {
        unlistenFocus();
        unlistenBlur();
      };
    };

    const cleanup = setup();

    return () => {
      unlistenTray.then((fn) => fn());
      cleanup.then((fn) => fn());
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
