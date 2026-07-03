import { useState } from "react";
import { useNoteStore } from "../stores/noteStore";

export default function TemplatePicker() {
  const {
    templates,
    templatePickerOpen,
    setTemplatePickerOpen,
    createNote,
    importNotes,
  } = useNoteStore();
  const [showPreset, setShowPreset] = useState(true);
  const [showCustom, setShowCustom] = useState(true);

  if (!templatePickerOpen) return null;

  const handlePick = (templateId?: string) => {
    createNote(templateId);
  };

  const handleImport = async () => {
    try {
      const count = await importNotes();
      if (count) {
        window.alert(`成功导入 ${count} 条便签`);
      }
    } catch (e) {
      window.alert("导入失败");
      console.error(e);
    } finally {
      setTemplatePickerOpen(false);
    }
  };

  const presets = templates
    .filter((t) => t.preset)
    .sort((a, b) => a.created_at.localeCompare(b.created_at));

  const customs = templates
    .filter((t) => !t.preset)
    .sort((a, b) => a.created_at.localeCompare(b.created_at));

  return (
    <>
      <div
        className="dialog-overlay"
        onClick={() => setTemplatePickerOpen(false)}
      />
      <div className="dialog-panel">
        <div className="dialog-header">
          <span>新建便签 — 选择模板</span>
          <button
            className="dialog-close"
            onClick={() => setTemplatePickerOpen(false)}
          >
            ✕
          </button>
        </div>
        <div className="dialog-body">
          {/* Preset group */}
          {presets.length > 0 && (
            <div className="template-group">
              <button
                className="template-group-header"
                onClick={() => setShowPreset(!showPreset)}
              >
                <span className="template-group-arrow">
                  {showPreset ? "▼" : "▶"}
                </span>
                <span>预设模板</span>
                <span className="template-group-count">{presets.length}</span>
              </button>
              {showPreset && (
                <div className="template-group-list">
                  {presets.map((t) => (
                    <button
                      key={t.id}
                      className="template-option"
                      onClick={() => handlePick(t.id)}
                    >
                      <div className="template-option-title">
                        📋 {t.name}
                        <span className="template-badge">预设</span>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Custom group */}
          {customs.length > 0 && (
            <div className="template-group">
              <button
                className="template-group-header"
                onClick={() => setShowCustom(!showCustom)}
              >
                <span className="template-group-arrow">
                  {showCustom ? "▼" : "▶"}
                </span>
                <span>自定义模板</span>
                <span className="template-group-count">{customs.length}</span>
              </button>
              {showCustom && (
                <div className="template-group-list">
                  {customs.map((t) => (
                    <button
                      key={t.id}
                      className="template-option"
                      onClick={() => handlePick(t.id)}
                    >
                      <div className="template-option-title">
                        📝 {t.name}
                      </div>
                    </button>
                  ))}
                </div>
              )}

          {/* Blank option */}
          <button
            className="template-option template-option-blank"
            onClick={() => handlePick(undefined)}
          >
            <div className="template-option-title">空白便签</div>
            <div className="template-option-desc">从零开始</div>
          </button>

          {/* Import option */}
          <button
            className="template-option template-option-import"
            onClick={handleImport}
          >
            <div className="template-option-title">📥 导入 Markdown</div>
            <div className="template-option-desc">从 .md 文件创建便签</div>
          </button>
            </div>
          )}
        </div>
      </div>
    </>
  );
}
