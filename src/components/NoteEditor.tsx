import { useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import { Crepe } from "@milkdown/crepe";
import { Milkdown, MilkdownProvider, useEditor } from "@milkdown/react";
import { useNoteStore } from "../stores/noteStore";
import { tomorrowCheckProse } from "./tomorrowCheck";
import { enterTaskResetProse } from "./enterTaskFix";
import "@milkdown/crepe/theme/common/style.css";
import "@milkdown/crepe/theme/frame.css";

/**
 * 解析链接目标：指向本地文件（file:// 前缀或 Windows 盘符绝对路径）时
 * 返回可被系统打开的实际路径，否则返回 null（web 链接等交给默认行为）。
 */
function resolveLocalPath(href: string): string | null {
  if (href.startsWith("file://")) {
    const raw = href.slice("file://".length);
    let path: string;
    try {
      path = decodeURIComponent(raw);
    } catch {
      path = raw;
    }
    // file:///D:/foo.pdf → D:/foo.pdf
    if (/^\/[A-Za-z]:[\\/]/.test(path)) path = path.slice(1);
    return path;
  }
  // 直接书写盘符绝对路径：D:\foo.pdf 或 D:/foo.pdf
  if (/^[A-Za-z]:[\\/]/.test(href)) return href;
  return null;
}

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
  // Ignore the first markdownUpdated: it fires once on editor init when the
  // loaded defaultValue is parsed — not a user edit. Without this, opening a
  // note triggers a no-op save that bumps updated_at / retitles daily todos.
  const isFirstUpdate = useRef(true);

  useEditor((root) => {
    const crepe = new Crepe({
      root,
      defaultValue,
    });

    // 明日委托勾选框（每条任务/引用块末尾）+ 回车新任务项勾选重置
    crepe.addFeature((editor) => {
      editor.use(tomorrowCheckProse);
      editor.use(enterTaskResetProse);
    });

    // Listen for changes
    crepe.on((listener) => {
      listener.markdownUpdated((_, md) => {
        if (isFirstUpdate.current) {
          isFirstUpdate.current = false;
          return;
        }
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

  // 单击编辑器内的本地文件链接时，用系统默认程序打开。
  // 用 document 捕获阶段监听（先于 Milkdown 的冒泡处理），仅处理 Milkdown 渲染区内的链接。
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (e.button !== 0) return;
      const target = e.target as HTMLElement | null;
      if (!target?.closest?.(".ProseMirror")) return; // 仅编辑器内容区
      const anchor = target.closest("a[href]");
      if (!anchor) return;
      const href = anchor.getAttribute("href") ?? "";
      const local = resolveLocalPath(href);
      if (!local) return; // 非本地文件链接，保留默认行为
      e.preventDefault();
      e.stopPropagation();
      // 非 Tauri（纯浏览器预览）环境下 openPath 不可用，静默忽略
      try {
        openPath(local).catch(() => {});
      } catch {
        // ignore
      }
    };

    document.addEventListener("click", onClick, true);
    return () => document.removeEventListener("click", onClick, true);
  }, []);

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
      // 标成「有未落盘改动」：桌宠在这 500ms 里不许缩回
      useNoteStore.setState({ hasPendingEdit: true });
      saveTimer.current = setTimeout(async () => {
        try {
          await invoke("save_note", { id: activeId, content: md });
          useNoteStore.setState({ hasPendingEdit: false });
          // 勾选变了就刷新当天进度：桌宠的动作与气泡要跟着当天的真实状态走
          void useNoteStore.getState().fetchProgress();
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
