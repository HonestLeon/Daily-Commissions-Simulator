import { getCurrentWindow } from "@tauri-apps/api/window";
import { useNoteStore } from "../stores/noteStore";

interface Props {
  onSettings: () => void;
}

export default function TitleBar({ onSettings }: Props) {
  const { isPinned, opacity, togglePin, setOpacity } = useNoteStore();

  const handlePin = async () => {
    const win = getCurrentWindow();
    const newPinned = !isPinned;
    await win.setAlwaysOnTop(newPinned);
    togglePin();
  };

  const handleOpacityChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = Number(e.target.value);
    setOpacity(val);
  };

  const handleHide = () => {
    getCurrentWindow().hide();
  };

  return (
    <div
      className="titlebar"
      data-tauri-drag-region
      onDoubleClick={(e) => {
        if ((e.target as HTMLElement).hasAttribute("data-tauri-drag-region")) {
          const win = getCurrentWindow();
          win.isMaximized().then((max) => {
            if (max) win.unmaximize();
            else win.maximize();
          });
        }
      }}
    >
      <span className="titlebar-title">轻量便签</span>
      <div style={{ flex: 1 }} data-tauri-drag-region />
      <div className="titlebar-controls">
        <button
          className={`titlebar-btn ${isPinned ? "active" : ""}`}
          onClick={handlePin}
          title={isPinned ? "取消置顶" : "窗口置顶"}
        >
          📌
        </button>

        <div className="titlebar-slider" title={`透明度: ${Math.round(opacity * 100)}%`}>
          <span style={{ fontSize: 12 }}>🎚</span>
          <input
            type="range"
            min="0.25"
            max="1"
            step="0.01"
            value={opacity}
            onChange={handleOpacityChange}
            style={{ width: 60, cursor: "pointer" }}
          />
        </div>

        <button className="titlebar-btn" onClick={onSettings} title="设置">
          ⚙
        </button>

        <button className="titlebar-btn" onClick={handleHide} title="隐藏到托盘">
          ─
        </button>

        <button className="titlebar-btn titlebar-btn-close" onClick={handleHide} title="关闭到托盘">
          ✕
        </button>
      </div>
    </div>
  );
}
