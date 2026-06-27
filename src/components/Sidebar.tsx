import { useNoteStore } from "../stores/noteStore";
import { useState, useRef, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { NoteMeta } from "../types";

const BLANK_GROUP_KEY = "__blank__";
const BLANK_GROUP_NAME = "空白便签";
const UNKNOWN_TEMPLATE_NAME = "未知模板";
const COLLAPSED_STORAGE_KEY = "sidebar-collapsed-groups";

export default function Sidebar() {
  const {
    notes,
    templates,
    activeId,
    setActive,
    setTemplatePickerOpen,
    deleteNote,
    renameNote,
    saveAsTemplate,
    exportNote,
  } = useNoteStore();
  const [collapsed, setCollapsed] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(() => {
    try {
      const raw = localStorage.getItem(COLLAPSED_STORAGE_KEY);
      return new Set(raw ? (JSON.parse(raw) as string[]) : []);
    } catch {
      return new Set<string>();
    }
  });
  const [contextMenu, setContextMenu] = useState<{
    id: string;
    x: number;
    y: number;
  } | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const [status, setStatus] = useState<string>("");
  const editInputRef = useRef<HTMLInputElement>(null);

  const filteredNotes = searchQuery.trim()
    ? notes.filter((note) =>
        note.title.toLowerCase().includes(searchQuery.toLowerCase().trim())
      )
    : notes;

  const groups = useMemo(() => {
    const map = new Map<string, NoteMeta[]>();
    filteredNotes.forEach((note) => {
      const key = note.template_id || BLANK_GROUP_KEY;
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(note);
    });

    return Array.from(map.entries())
      .map(([key, notes]) => {
        const name =
          key === BLANK_GROUP_KEY
            ? BLANK_GROUP_NAME
            : templates.find((t) => t.id === key)?.name || UNKNOWN_TEMPLATE_NAME;
        return { key, name, notes };
      })
      .sort((a, b) => {
        if (a.key === BLANK_GROUP_KEY) return -1;
        if (b.key === BLANK_GROUP_KEY) return 1;
        return a.name.localeCompare(b.name, "zh-CN");
      });
  }, [filteredNotes, templates]);

  useEffect(() => {
    if (editingId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingId]);

  const toggleGroup = (key: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      localStorage.setItem(COLLAPSED_STORAGE_KEY, JSON.stringify([...next]));
      return next;
    });
  };

  const isGroupCollapsed = (key: string) => collapsedGroups.has(key);

  const handleCreate = () => {
    setTemplatePickerOpen(true);
  };

  const handleRightClick = (id: string, e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ id, x: e.clientX, y: e.clientY });
  };

  const handleDelete = async (id: string) => {
    await deleteNote(id);
    setContextMenu(null);
  };

  const handleSaveAsTemplate = async (id: string) => {
    try {
      const content: string = await invoke("load_note", { id });
      const note = notes.find((n) => n.id === id);
      const name = note?.title || "未命名模板";
      await saveAsTemplate(name, content);
    } catch (e) {
      console.error("Failed to save as template:", e);
    }
    setContextMenu(null);
  };

  const showStatus = (message: string) => {
    setStatus(message);
    setTimeout(() => setStatus(""), 3000);
  };

  const handleExport = async (id: string) => {
    try {
      const path = await exportNote(id);
      if (path) {
        showStatus(`已导出: ${path}`);
      }
    } catch (e) {
      showStatus("导出失败");
      console.error(e);
    }
    setContextMenu(null);
  };

  const startRename = (id: string, title: string) => {
    setEditingId(id);
    setEditTitle(title);
    setContextMenu(null);
  };

  const confirmRename = async () => {
    if (editingId && editTitle.trim()) {
      await renameNote(editingId, editTitle.trim());
    }
    setEditingId(null);
    setEditTitle("");
  };

  const handleRenameKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      confirmRename();
    } else if (e.key === "Escape") {
      setEditingId(null);
      setEditTitle("");
    }
  };

  const formatDate = (iso: string) => {
    try {
      const d = new Date(iso);
      const now = new Date();
      const diff = now.getTime() - d.getTime();
      if (diff < 86400000) {
        return d.toLocaleTimeString("zh-CN", {
          hour: "2-digit",
          minute: "2-digit",
        });
      }
      return d.toLocaleDateString("zh-CN", {
        month: "short",
        day: "numeric",
      });
    } catch {
      return "";
    }
  };

  const renderNoteItem = (note: NoteMeta) => (
    <div
      key={note.id}
      className={`sidebar-item ${note.id === activeId ? "active" : ""}`}
      onClick={() => setActive(note.id)}
      onContextMenu={(e) => handleRightClick(note.id, e)}
      onDoubleClick={() => startRename(note.id, note.title)}
    >
      {editingId === note.id ? (
        <input
          ref={editInputRef}
          className="sidebar-rename-input"
          value={editTitle}
          onChange={(e) => setEditTitle(e.target.value)}
          onKeyDown={handleRenameKeyDown}
          onBlur={confirmRename}
          onClick={(e) => e.stopPropagation()}
        />
      ) : (
        <>
          <div className="sidebar-item-title">
            {note.title || "无标题便签"}
          </div>
          <div className="sidebar-item-date">
            {formatDate(note.updated_at)}
          </div>
        </>
      )}
    </div>
  );

  return (
    <>
      <div className={`sidebar ${collapsed ? "collapsed" : ""}`}>
        <div className="sidebar-header">
          {!collapsed && <span className="sidebar-title">便签列表</span>}
          <button
            className="sidebar-toggle"
            onClick={() => setCollapsed(!collapsed)}
            title={collapsed ? "展开" : "折叠"}
          >
            {collapsed ? "☰" : "◀"}
          </button>
        </div>

        {!collapsed && (
          <>
            <div className="sidebar-search">
              <input
                type="text"
                className="sidebar-search-input"
                placeholder="搜索便签..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>

            <div className="sidebar-list">
              {groups.map((group) => {
                const expanded = !isGroupCollapsed(group.key);
                return (
                  <div key={group.key} className="sidebar-group">
                    <div
                      className="sidebar-group-header"
                      onClick={() => toggleGroup(group.key)}
                    >
                      <span className="sidebar-group-toggle">
                        {expanded ? "▼" : "▶"}
                      </span>
                      <span className="sidebar-group-name">{group.name}</span>
                      <span className="sidebar-group-count">
                        {group.notes.length}
                      </span>
                    </div>
                    {expanded && (
                      <div className="sidebar-group-items">
                        {group.notes.map(renderNoteItem)}
                      </div>
                    )}
                  </div>
                );
              })}
              {groups.length === 0 && (
                <div className="sidebar-empty">
                  {searchQuery.trim() ? "无匹配便签" : "暂无便签"}
                </div>
              )}
            </div>

            {status && (
              <div className="sidebar-status" title={status}>
                {status}
              </div>
            )}
            <div className="sidebar-footer">
              <button className="sidebar-create-btn" onClick={handleCreate}>
                ＋ 新建便签
              </button>
            </div>
          </>
        )}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <>
          <div
            className="context-overlay"
            onClick={() => setContextMenu(null)}
          />
          <div
            className="context-menu"
            style={{ left: contextMenu.x, top: contextMenu.y }}
          >
            <button
              onClick={() =>
                startRename(
                  contextMenu.id,
                  notes.find((n) => n.id === contextMenu.id)?.title ?? ""
                )
              }
            >
              重命名
            </button>
            <button onClick={() => handleSaveAsTemplate(contextMenu.id)}>
              保存为模板
            </button>
            <button onClick={() => handleExport(contextMenu.id)}>
              导出 Markdown
            </button>
            <button onClick={() => handleDelete(contextMenu.id)}>
              删除便签
            </button>
          </div>
        </>
      )}
    </>
  );
}
