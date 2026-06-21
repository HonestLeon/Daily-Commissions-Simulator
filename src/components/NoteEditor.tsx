import { useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Crepe } from "@milkdown/crepe";
import { Milkdown, MilkdownProvider, useEditor } from "@milkdown/react";
import { useNoteStore } from "../stores/noteStore";
import "@milkdown/crepe/theme/common/style.css";
import "@milkdown/crepe/theme/frame.css";

// Inner editor component — uses Milkdown hooks
function InnerEditor({
  defaultValue,
  onChange,
}: {
  defaultValue: string;
  onChange: (md: string) => void;
}) {
  const editorRef = useRef<Crepe | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEditor((root) => {
    const crepe = new Crepe({
      root,
      defaultValue,
    });

    // Listen for changes
    crepe.on((listener) => {
      listener.markdownUpdated((_, md) => {
        onChangeRef.current(md);
      });
    });

    editorRef.current = crepe;
    return crepe;
  }, []);

  return <Milkdown />;
}

export default function NoteEditor() {
  const { activeId } = useNoteStore();
  const [markdown, setMarkdown] = useState("");
  const [loading, setLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState("");
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const editorKey = useRef(0);

  // Load note content when activeId changes
  useEffect(() => {
    if (!activeId) {
      setMarkdown("");
      return;
    }
    setLoading(true);
    invoke<string>("load_note", { id: activeId })
      .then((c) => {
        setMarkdown(c);
        editorKey.current += 1;
        setLoading(false);
      })
      .catch((e) => {
        console.error("Failed to load note:", e);
        setMarkdown("");
        editorKey.current += 1;
        setLoading(false);
      });
  }, [activeId]);

  // Auto-save with debounce
  const handleChange = useCallback(
    (md: string) => {
      if (!activeId) return;
      setMarkdown(md);
      setSaveStatus("未保存...");

      if (saveTimer.current) clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(async () => {
        try {
          await invoke("save_note", { id: activeId, content: md });
          setSaveStatus("已保存");
          setTimeout(() => setSaveStatus(""), 2000);
        } catch (e) {
          setSaveStatus("保存失败");
          console.error(e);
        }
      }, 500);
    },
    [activeId]
  );

  if (!activeId) {
    return (
      <div className="editor-placeholder">
        <p>选择或创建一个便签开始编辑</p>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="editor-placeholder">
        <p>加载中...</p>
      </div>
    );
  }

  return (
    <div className="editor-container">
      <div className="milkdown-wrapper" key={editorKey.current}>
        <MilkdownProvider>
          <InnerEditor defaultValue={markdown} onChange={handleChange} />
        </MilkdownProvider>
      </div>
      {saveStatus && <div className="editor-status">{saveStatus}</div>}
    </div>
  );
}
