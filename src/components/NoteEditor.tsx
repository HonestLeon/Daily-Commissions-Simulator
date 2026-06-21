import { useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNoteStore } from "../stores/noteStore";

export default function NoteEditor() {
  const { activeId } = useNoteStore();
  const [content, setContent] = useState("");
  const [loading, setLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState("");
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load note content when activeId changes
  useEffect(() => {
    if (!activeId) {
      setContent("");
      return;
    }
    setLoading(true);
    invoke<string>("load_note", { id: activeId })
      .then((c) => {
        setContent(c);
        setLoading(false);
      })
      .catch((e) => {
        console.error("Failed to load note:", e);
        setContent("");
        setLoading(false);
      });
  }, [activeId]);

  // Auto-save with debounce
  const save = useCallback(
    (text: string, id: string) => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(async () => {
        try {
          await invoke("save_note", { id, content: text });
          setSaveStatus("已保存");
          setTimeout(() => setSaveStatus(""), 2000);
        } catch (e) {
          setSaveStatus("保存失败");
          console.error(e);
        }
      }, 500);
    },
    []
  );

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setContent(text);
    if (activeId) {
      save(text, activeId);
      setSaveStatus("未保存...");
    }
  };

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
      <textarea
        className="editor-textarea"
        value={content}
        onChange={handleChange}
        placeholder="开始输入 Markdown..."
        spellCheck={false}
      />
      {saveStatus && <div className="editor-status">{saveStatus}</div>}
    </div>
  );
}
