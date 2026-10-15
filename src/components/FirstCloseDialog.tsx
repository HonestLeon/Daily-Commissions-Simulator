interface Props {
  open: boolean;
  onSelect: (closeToTray: boolean) => void;
}

export default function FirstCloseDialog({ open, onSelect }: Props) {
  if (!open) return null;

  return (
    <>
      <div className="dialog-overlay" />
      <div className="dialog-panel" style={{ width: 340 }}>
        <div className="dialog-header">
          <span>关闭行为</span>
        </div>
        <div className="dialog-body">
          <p style={{ fontSize: 14, color: "#888", marginTop: 0 }}>
            请选择点击关闭按钮时的默认行为：
          </p>
          <button className="template-option" onClick={() => onSelect(true)}>
            <div className="template-option-title">📌 最小化到托盘</div>
            <div className="template-option-desc">
              关闭窗口后应用继续在后台运行，可随时从托盘打开
            </div>
          </button>
          <button className="template-option" onClick={() => onSelect(false)}>
            <div className="template-option-title">🚪 直接退出程序</div>
            <div className="template-option-desc">关闭窗口时完全退出应用</div>
          </button>
          <p style={{ fontSize: 12, color: "#aaa", margin: "12px 0 0" }}>
            后续可在「设置」中随时修改。
          </p>
        </div>
      </div>
    </>
  );
}
