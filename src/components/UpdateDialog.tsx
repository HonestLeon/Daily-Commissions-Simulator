import type { UpdateInfo } from "../types";

export type UpdateState =
  | { phase: "checking" }
  | { phase: "error"; message: string }
  | { phase: "result"; info: UpdateInfo }
  | { phase: "installing" }
  | { phase: "done" };

interface Props {
  state: UpdateState | null;
  onClose: () => void;
  onUpdate: () => void;
}

export default function UpdateDialog({ state, onClose, onUpdate }: Props) {
  if (!state) return null;

  // While checking or installing, don't let the user dismiss mid-flight.
  const closable = state.phase !== "checking" && state.phase !== "installing";

  const hint = (text: string, color = "#888") => (
    <p style={{ fontSize: 14, color, marginTop: 0 }}>{text}</p>
  );

  return (
    <>
      <div className="dialog-overlay" onClick={closable ? onClose : undefined} />
      <div className="dialog-panel" style={{ width: 340 }}>
        <div className="dialog-header">
          <span>检查更新</span>
          {closable && (
            <button className="dialog-close" onClick={onClose}>
              ✕
            </button>
          )}
        </div>
        <div className="dialog-body">
          {state.phase === "checking" && hint("正在检查更新…")}
          {state.phase === "installing" && hint("正在下载并安装更新，请稍候…")}
          {state.phase === "done" && hint("更新完成！请重新启动应用。")}
          {state.phase === "error" && hint(state.message, "#c0392b")}
          {state.phase === "result" &&
            !state.info.has_update &&
            hint(`已是最新版本（v${state.info.current_version}）`)}
          {state.phase === "result" && state.info.has_update && (
            <>
              <p style={{ fontSize: 14, color: "#888", marginTop: 0 }}>
                发现新版本 <strong>{state.info.latest_version}</strong>
                <br />
                （当前版本 v{state.info.current_version}）
              </p>
              <button className="template-option" onClick={onUpdate}>
                <div className="template-option-title">⬇️ 立即更新</div>
                <div className="template-option-desc">自动下载并安装新版本</div>
              </button>
            </>
          )}
        </div>
      </div>
    </>
  );
}
