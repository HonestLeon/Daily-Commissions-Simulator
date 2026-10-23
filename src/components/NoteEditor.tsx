import { useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { Crepe } from "@milkdown/crepe";
import { linkSchema } from "@milkdown/preset-commonmark";
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

/**
 * Milkdown 7.22 的 link schema 会把 file: 与盘符路径当成不安全 scheme 清洗成空 href，
 * 而本应用需要“单击本地文件交给系统打开”。这里只给这两类本地路径恢复原始 href；
 * javascript:/data:/vbscript: 等仍走 Milkdown 原来的清洗结果。
 */
const localLinkSchema = linkSchema.extendSchema((prev) => (ctx) => {
  const spec = prev(ctx);
  return {
    ...spec,
    toDOM: (mark, inline) => {
      const dom = spec.toDOM?.(mark, inline);
      if (!dom) return ["span", {}];
      const href = typeof mark.attrs.href === "string" ? mark.attrs.href : "";
      if (!resolveLocalPath(href) || !Array.isArray(dom)) return dom;
      const [tag, attrs, ...children] = dom as [
        string,
        Record<string, unknown> | null | undefined,
        ...unknown[],
      ];
      return [tag, { ...(attrs ?? {}), href }, ...children];
    },
  };
});

interface PendingSave {
  content: string;
  token: number;
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

    // 本地文件链接需要原始 href；其余链接仍由 Milkdown 的清洗函数保护。
    // 明日委托勾选框（每条任务/引用块末尾）+ 回车新任务项勾选重置
    crepe.addFeature((editor) => {
      editor.use(localLinkSchema);
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
  // 当前编辑器里装的是哪张便签。切换 activeId 的那一帧就靠它卸下旧编辑器，
  // 不等 effect：否则旧编辑器晚到的 markdownUpdated 可能用新 activeId 的
  // onChange 闭包，把 A 的内容写进 B。
  const [loadedId, setLoadedId] = useState<string | null>(null);
  const [saveStatus, setSaveStatus] = useState("");
  const editorKey = useRef(0);
  const activeIdRef = useRef<string | null>(null);
  const statusTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // 保存按便签 id 分开排队：切换便签时不能把上一张的定时器清掉（会丢最后 500ms）。
  const pendingRef = useRef<Map<string, PendingSave>>(new Map());
  const timersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const savingRef = useRef<Set<string>>(new Set());
  const tokenRef = useRef(0);
  const savePendingRef = useRef<(id: string) => Promise<void>>(async () => {});

  const clearTimer = useCallback((id: string) => {
    const timer = timersRef.current.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      timersRef.current.delete(id);
    }
  }, []);

  const syncPendingFlag = useCallback(() => {
    useNoteStore.setState({ hasPendingEdit: pendingRef.current.size > 0 });
  }, []);

  const scheduleSave = useCallback(
    (id: string, delay: number) => {
      clearTimer(id);
      timersRef.current.set(
        id,
        setTimeout(() => {
          timersRef.current.delete(id);
          void savePendingRef.current(id);
        }, delay)
      );
    },
    [clearTimer]
  );

  /**
   * 落盘一条待保存内容。pending 在成功前一直保留：失败会 5 秒后重试，
   * 期间切走便签也不会丢；新编辑会用自己的 token 覆盖旧内容。
   */
  const savePending = useCallback(
    async (id: string) => {
      const pending = pendingRef.current.get(id);
      if (!pending) return;
      if (savingRef.current.has(id)) {
        // 同一张便签已有保存在飞，等它结束后再处理最新内容
        if (!timersRef.current.has(id)) scheduleSave(id, 300);
        return;
      }

      clearTimer(id);
      savingRef.current.add(id);
      try {
        await invoke("save_note", { id, content: pending.content });
        if (pendingRef.current.get(id)?.token === pending.token) {
          pendingRef.current.delete(id);
        }
        syncPendingFlag();
        // 勾选变了就刷新当天进度：桌宠的动作与气泡要跟着当天的真实状态走
        void useNoteStore.getState().fetchProgress();
        if (id === activeIdRef.current) {
          setSaveStatus("已保存");
          if (statusTimer.current) clearTimeout(statusTimer.current);
          statusTimer.current = setTimeout(() => setSaveStatus(""), 2000);
        }
      } catch (e) {
        console.error("保存失败:", e);
        if (id === activeIdRef.current) setSaveStatus("保存失败");
        // 内容还在 pending 里；只有没有人顶掉它时才排重试
        if (pendingRef.current.get(id)?.token === pending.token) {
          scheduleSave(id, 5000);
        }
      } finally {
        savingRef.current.delete(id);
      }
    },
    [clearTimer, scheduleSave, syncPendingFlag]
  );
  savePendingRef.current = savePending;

  // 单击编辑器内的链接时接管：
  // - 本地文件交给系统默认程序；
  // - http(s)/mailto/tel 交给系统浏览器；
  // - 其余（相对路径、被清洗后留下的空 href 等）阻止 WebView 导航离开应用。
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (e.button !== 0) return;
      const target = e.target as HTMLElement | null;
      if (!target?.closest?.(".ProseMirror")) return; // 仅编辑器内容区
      const anchor = target.closest("a[href]");
      if (!anchor) return;
      const href = anchor.getAttribute("href") ?? "";
      e.preventDefault();
      e.stopPropagation();

      const local = resolveLocalPath(href);
      if (local) {
        try {
          openPath(local).catch(() => {});
        } catch {
          // 非 Tauri（纯浏览器预览）环境下 openPath 不可用，静默忽略
        }
        return;
      }
      if (/^(https?:|mailto:|tel:)/i.test(href)) {
        try {
          openUrl(href).catch(() => {});
        } catch {
          // 非 Tauri 环境
        }
      }
    };

    document.addEventListener("click", onClick, true);
    return () => document.removeEventListener("click", onClick, true);
  }, []);

  // Load note content when activeId changes.
  // 切换前先把上一张的待保存内容排进落盘队列；不能在这里取消它的定时器。
  useEffect(() => {
    const prevId = activeIdRef.current;
    activeIdRef.current = activeId;

    if (prevId && prevId !== activeId) {
      const stillExists = useNoteStore.getState().notes.some((n) => n.id === prevId);
      if (!stillExists) {
        // 便签已被删除：丢弃它的待保存内容，不能把 .md 重新写回来
        clearTimer(prevId);
        pendingRef.current.delete(prevId);
        syncPendingFlag();
      } else if (pendingRef.current.has(prevId)) {
        void savePending(prevId);
      }
    }

    if (statusTimer.current) {
      clearTimeout(statusTimer.current);
      statusTimer.current = null;
    }
    setSaveStatus("");

    if (!activeId) {
      setMarkdown("");
      setLoadedId(null);
      return;
    }

    const pending = pendingRef.current.get(activeId);
    if (pending) {
      // 切回一张还有未落盘内容的便签：直接显示内存里的新内容，别用旧文件覆盖
      setMarkdown(pending.content);
      setLoadedId(activeId);
      useNoteStore.setState({ hasPendingEdit: true });
      editorKey.current += 1;
      scheduleSave(activeId, 500);
      return;
    }

    let dead = false;
    invoke<string>("load_note", { id: activeId })
      .then((c) => {
        if (dead) return;
        // 加载期间旧编辑器可能又吐了一次更新，内存内容仍是最新的
        const latest = pendingRef.current.get(activeId);
        setMarkdown(latest ? latest.content : c);
        setLoadedId(activeId);
        editorKey.current += 1;
      })
      .catch((e) => {
        if (dead) return;
        console.error("Failed to load note:", e);
        setMarkdown("");
        setLoadedId(activeId);
        editorKey.current += 1;
      });
    return () => {
      dead = true;
    };
  }, [activeId, clearTimer, savePending, scheduleSave, syncPendingFlag]);

  // Auto-save with debounce. 只给活跃便签更新 UI；旧编辑器卸下时的最后一次
  // markdownUpdated 仍要排进保存队列，但不能改动当前便签的 markdown/状态。
  const handleChange = useCallback(
    (md: string) => {
      const id = activeId;
      if (!id) return;
      // 便签已被删除时，旧编辑器卸载前的最后一次回调不能再把 .md 写回来
      if (!useNoteStore.getState().notes.some((n) => n.id === id)) return;
      const isActive = activeIdRef.current === id;
      if (isActive) {
        setMarkdown(md);
        setSaveStatus("未保存...");
        if (statusTimer.current) clearTimeout(statusTimer.current);
      }

      const token = ++tokenRef.current;
      pendingRef.current.set(id, { content: md, token });
      // 只有从 false 变 true 才通知；否则 App 会因为整店订阅在每次按键时重渲染
      if (!useNoteStore.getState().hasPendingEdit) {
        useNoteStore.setState({ hasPendingEdit: true });
      }
      scheduleSave(id, 500);
    },
    [activeId, scheduleSave]
  );

  useEffect(() => {
    return () => {
      for (const timer of timersRef.current.values()) clearTimeout(timer);
      timersRef.current.clear();
      if (statusTimer.current) clearTimeout(statusTimer.current);
    };
  }, []);

  if (!activeId) {
    return (
      <div className="editor-placeholder">
        <p>选择或创建一个便签开始编辑</p>
      </div>
    );
  }

  if (loadedId !== activeId) {
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
