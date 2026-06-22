import { useNoteStore } from "../stores/noteStore";
import { useState, useRef, useEffect } from "react";

export default function Sidebar() {
  const { notes, activeId, setActive, createNote, deleteNote, renameNote } =
    useNoteStore();
  const [collapsed, setCollapsed] = useState(false);
  const [contextMenu, setContextMenu] = useState<{
    id: string;
    x: number;
    y: number;
  } | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const editInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editingId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingId]);

  const handleCreate = async () => {
    await createNote();
  };

  const handleRightClick = (id: string, e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ id, x: e.clientX, y: e.clientY });
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
            <div className="sidebar-list">
              {notes.map((note) => (
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
              ))}
              {notes.length === 0 && (
                <div className="sidebar-empty">暂无便签</div>
              )}
            </div>

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
            <button onClick={() => handleDelete(contextMenu.id)}>
              删除便签
            </button>
          </div>
        </>
      )}
    </>
  );
}
