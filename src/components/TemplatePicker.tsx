import { useNoteStore } from "../stores/noteStore";

export default function TemplatePicker() {
  const { templates, templatePickerOpen, setTemplatePickerOpen, createNote } =
    useNoteStore();

  if (!templatePickerOpen) return null;

  const handlePick = (templateId?: string) => {
    createNote(templateId);
  };

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
          <button
            className="template-option template-option-blank"
            onClick={() => handlePick(undefined)}
          >
            <div className="template-option-title">空白便签</div>
            <div className="template-option-desc">从零开始</div>
          </button>
          {templates.map((t) => (
            <button
              key={t.id}
              className="template-option"
              onClick={() => handlePick(t.id)}
            >
              <div className="template-option-title">
                {t.preset ? "📋 " : "📝 "}
                {t.name}
                {t.preset && (
                  <span className="template-badge">预设</span>
                )}
              </div>
            </button>
          ))}
        </div>
      </div>
    </>
  );
}
