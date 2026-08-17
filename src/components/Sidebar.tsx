import { useNoteStore } from "../stores/noteStore";
import { useState, useRef, useEffect } from "react";
import type { NoteMeta } from "../types";

export default function Sidebar() {
  const { notes, activeId, setActive, deleteNote, renameNote } = useNoteStore();
  const [collapsed, setCollapsed] = useState(false);
  const [contextMenu, setContextMenu] = useState<{
    id: string;
    x: number;
    y: number;
    above: boolean;
  } | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const editInputRef = useRef<HTMLInputElement>(null);

  // Newest first.
  const sortedNotes = [...notes].sort((a, b) => b.created_at.localeCompare(a.created_at));

  useEffect(() => {
    if (editingId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingId]);

  const handleRightClick = (id: string, e: React.MouseEvent) => {
    e.preventDefault();
    const estimatedHeight = 90;
    const above = e.clientY + estimatedHeight > window.innerHeight;
    setContextMenu({ id, x: e.clientX, y: e.clientY, above });
  };

  const handleDelete = async (id: string) => {
    await deleteNote(id);
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
      return d.toLocaleDateString("zh-CN", {
        month: "2-digit",
        day: "2-digit",
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
          <div className="sidebar-item-title">{note.title || "(无标题)"}</div>
          <div className="sidebar-item-date">{formatDate(note.created_at)}</div>
        </>
      )}
    </div>
  );

  return (
    <>
      <div className={`sidebar ${collapsed ? "collapsed" : ""}`}>
        <div className="sidebar-header">
          {!collapsed && <span className="sidebar-title">委托列表</span>}
          <button
            className="sidebar-toggle"
            onClick={() => setCollapsed(!collapsed)}
            title={collapsed ? "展开" : "折叠"}
          >
            {collapsed ? "☰" : "◀"}
          </button>
        </div>

        {!collapsed && (
          <div className="sidebar-list">
            {sortedNotes.length === 0 ? (
              <div className="sidebar-empty">暂无委托</div>
            ) : (
              sortedNotes.map(renderNoteItem)
            )}
          </div>
        )}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <>
          <div className="context-overlay" onClick={() => setContextMenu(null)} />
          <div
            className="context-menu"
            style={{
              left: contextMenu.x,
              ...(contextMenu.above
                ? { bottom: window.innerHeight - contextMenu.y }
                : { top: contextMenu.y }),
            }}
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
            <button onClick={() => handleDelete(contextMenu.id)}>删除委托</button>
          </div>
        </>
      )}
    </>
  );
}
