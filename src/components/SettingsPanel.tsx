import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNoteStore } from "../stores/noteStore";
import type { AppTheme, PetConfig } from "../types";

const THEME_OPTIONS: { id: AppTheme; name: string; bg: string; accent: string }[] = [
  { id: "light", name: "默认", bg: "#ffffff", accent: "#4a9eff" },
  { id: "sky", name: "天空蓝", bg: "#d2e2f6", accent: "#3d8bfd" },
  { id: "sakura", name: "樱花粉", bg: "#f6dde8", accent: "#d95f97" },
  { id: "green", name: "护眼绿", bg: "#d2e8d0", accent: "#4a9e6f" },
];

// 动作间隔六档。档位名不写死秒数——区间表只在 Rust 有一份（PET_ACTION_RANGES），
// 具体秒数展示用后端回传的 action_min_sec / action_max_sec。
const PET_ACTION_OPTIONS = ["很频繁", "频繁", "标准", "悠闲", "安静", "很安静"];

// 与 Rust 的 PET_SCALE_MIN/MAX 一致：宠物窗等于可见包围盒，缩放直接是桌面死区的尺寸
const PET_SCALE_MIN = 0.5;
const PET_SCALE_MAX = 1.5;

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function SettingsPanel({ open, onClose }: Props) {
  const {
    opacity,
    skinImage,
    skinActive,
    theme,
    // 动作间隔的实际秒数只用于展示：区间表只在 Rust 有一份，本地不复制
    petActionMinSec,
    petActionMaxSec,
    petEnabled,
    petScale,
    petActionIndex,
    petAutoCollapseMs,
    petHideOnFullscreen,
    setOpacity,
    importSkin,
    removeSkin,
    setTheme,
    setPetConfig,
  } = useNoteStore();
  const [autoStart, setAutoStart] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // 桌宠分区用本地草稿：滑块每动一帧就 invoke 会打爆后端（且每次都会重建托盘菜单），
  // 所以攒 400 ms 写一次；关面板时若有未落盘的改动则立即补一次。
  const [petDraft, setPetDraft] = useState<PetConfig | null>(null);
  const petDirty = useRef(false);

  useEffect(() => {
    if (!open) return;
    petDirty.current = false;
    const s = useNoteStore.getState();
    setPetDraft({
      petEnabled: s.petEnabled,
      scale: s.petScale,
      actionIndex: s.petActionIndex,
      autoCollapseMs: s.petAutoCollapseMs,
      hideOnFullscreen: s.petHideOnFullscreen,
    });
  }, [open]);

  // 托盘切换桌宠/其他 ui-state-changed 会改 store；面板没在编辑草稿时同步过来，
  // 否则下一次动滑块会用旧草稿把托盘的开关覆盖回去。
  useEffect(() => {
    if (!open || petDirty.current) return;
    setPetDraft({
      petEnabled,
      scale: petScale,
      actionIndex: petActionIndex,
      autoCollapseMs: petAutoCollapseMs,
      hideOnFullscreen: petHideOnFullscreen,
    });
  }, [open, petEnabled, petScale, petActionIndex, petAutoCollapseMs, petHideOnFullscreen]);

  useEffect(() => {
    if (!petDraft || !petDirty.current) return;
    const timer = setTimeout(() => {
      petDirty.current = false;
      void setPetConfig(petDraft);
    }, 400);
    return () => clearTimeout(timer);
  }, [petDraft, setPetConfig]);

  const patchPet = (patch: Partial<PetConfig>) => {
    petDirty.current = true;
    setPetDraft((d) => (d ? { ...d, ...patch } : d));
  };

  const closePanel = () => {
    if (petDirty.current && petDraft) {
      petDirty.current = false;
      void setPetConfig(petDraft);
    }
    onClose();
  };

  useEffect(() => {
    if (open) {
      invoke<boolean>("is_autostart_enabled_cmd")
        .then(setAutoStart)
        .catch(() => setAutoStart(false));
    }
  }, [open]);

  const toggleAutoStart = async () => {
    try {
      if (autoStart) {
        await invoke("disable_autostart_cmd");
        setAutoStart(false);
      } else {
        await invoke("enable_autostart_cmd");
        setAutoStart(true);
      }
    } catch (e) {
      window.alert(typeof e === "string" ? e : "切换开机自启失败");
      console.error("Failed to toggle autostart:", e);
    }
  };

  const handleSkinFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = ""; // 允许重复选择同一文件
    if (!file) return;
    try {
      const dataUrl = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(reader.result as string);
        reader.onerror = () => reject(reader.error);
        reader.readAsDataURL(file);
      });
      await importSkin(dataUrl);
    } catch (err) {
      window.alert(typeof err === "string" ? err : "导入皮肤失败");
      console.error("Failed to import skin:", err);
    }
  };

  if (!open) return null;

  return (
    <>
      <div className="settings-overlay" onClick={closePanel} />
      <div className="settings-panel">
        <div className="settings-header">
          <span>设置</span>
          <button className="settings-close" onClick={closePanel}>
            ✕
          </button>
        </div>
        <div className="settings-body">
          {/* Auto-start toggle */}
          <button
            type="button"
            role="switch"
            aria-checked={autoStart}
            className="settings-row"
            onClick={toggleAutoStart}
          >
            <div className="settings-label">
              <div className="settings-label-title">开机自启动</div>
              <div className="settings-label-desc">登录 Windows 时自动启动应用并直接显示窗口</div>
            </div>
            <div className={`settings-toggle ${autoStart ? "on" : "off"}`}>
              <div className="settings-toggle-knob" />
            </div>
          </button>

          {/* Theme selection */}
          <div className="settings-section-title">主题</div>
          <div className="theme-options" role="radiogroup" aria-label="界面主题">
            {THEME_OPTIONS.map((t) => (
              <button
                key={t.id}
                type="button"
                role="radio"
                aria-checked={theme === t.id}
                className={`theme-card${theme === t.id ? " active" : ""}`}
                onClick={() => setTheme(t.id)}
              >
                <div className="theme-card-preview" style={{ background: t.bg }}>
                  <span className="theme-card-sample">委托 Aa</span>
                  <span className="theme-card-bar" style={{ background: t.accent }} />
                </div>
                <span className="theme-card-name">{t.name}</span>
              </button>
            ))}
          </div>

          {/* Window opacity */}
          <div className="settings-section-title">显示</div>
          <label className="settings-row">
            <div className="settings-label">
              <div className="settings-label-title">透明度（{Math.round(opacity * 100)}%）</div>
              <div className="settings-label-desc">窗口整体不透明度（25% ~ 100%）</div>
            </div>
            <input
              type="range"
              className="settings-range"
              min="0.25"
              max="1"
              step="0.01"
              value={opacity}
              onChange={(e) => setOpacity(Number(e.target.value))}
              aria-label="窗口透明度"
            />
          </label>

          {/* Skin section */}
          <div className="settings-section-title">皮肤</div>
          <div className="settings-row">
            <div className="settings-label">
              <div className="settings-label-title">导入皮肤</div>
              <div className="settings-label-desc">
                整窗背景图（各颜色主题下均可用，支持 PNG / JPG / WebP，≤8MB）
              </div>
            </div>
            {skinImage && (
              <div
                className="settings-skin-preview"
                style={{ backgroundImage: `url(${skinImage})` }}
              />
            )}
          </div>
          <div className="settings-row">
            <button className="settings-action-btn" onClick={() => fileInputRef.current?.click()}>
              导入 / 更换
            </button>
            {skinActive && (
              <button className="settings-action-btn danger" onClick={removeSkin}>
                移除皮肤
              </button>
            )}
          </div>
          <input
            ref={fileInputRef}
            type="file"
            accept=".png,.jpg,.jpeg,.webp,image/png,image/jpeg,image/webp"
            style={{ display: "none" }}
            onChange={handleSkinFile}
          />

          {/* 关闭行为不再做成设置项：一律「关闭即退出」，托盘只用于显示窗口与切桌宠/自启 */}

          {/* 桌宠分区 */}
          <div className="settings-section-title">桌宠</div>
          <button
            type="button"
            role="switch"
            aria-checked={!!petDraft?.petEnabled}
            className="settings-row"
            onClick={() => patchPet({ petEnabled: !petDraft?.petEnabled })}
          >
            <div className="settings-label">
              <div className="settings-label-title">桌宠模式</div>
              <div className="settings-label-desc">
                鼠标移开便签后缩成一只常驻桌面的小宠物；单击它展开今日委托。关闭后行为与之前完全一致
              </div>
            </div>
            <div className={`settings-toggle ${petDraft?.petEnabled ? "on" : "off"}`}>
              <div className="settings-toggle-knob" />
            </div>
          </button>

          {petDraft?.petEnabled && (
            <>
              <label className="settings-row">
                <div className="settings-label">
                  <div className="settings-label-title">
                    宠物大小（{Math.round((petDraft?.scale ?? 1) * 100)}%）
                  </div>
                  <div className="settings-label-desc">
                    窗口就是宠物可见轮廓的外框，放大它等于放大桌面上点不动的一圈死区，故上限 150%
                  </div>
                </div>
                <input
                  type="range"
                  className="settings-range"
                  min={PET_SCALE_MIN}
                  max={PET_SCALE_MAX}
                  step={0.05}
                  value={petDraft.scale}
                  onChange={(e) => patchPet({ scale: Number(e.target.value) })}
                  aria-label="宠物大小"
                />
              </label>

              <div className="settings-row">
                <div className="settings-label">
                  <div className="settings-label-title">动作间隔</div>
                  <div className="settings-label-desc">
                    当前生效 {petActionMinSec}–{petActionMaxSec}{" "}
                    秒，且在区间内随机（固定间隔会被摸出规律）
                  </div>
                </div>
              </div>
              <div className="pet-chip-row" role="radiogroup" aria-label="动作间隔档位">
                {PET_ACTION_OPTIONS.map((name, index) => (
                  <button
                    key={name}
                    type="button"
                    role="radio"
                    aria-checked={petDraft.actionIndex === index}
                    className={`pet-chip${petDraft.actionIndex === index ? " active" : ""}`}
                    onClick={() => patchPet({ actionIndex: index })}
                  >
                    {name}
                  </button>
                ))}
              </div>

              <div className="settings-row">
                <div className="settings-label">
                  <div className="settings-label-title">缩回宽限（毫秒，0 = 不自动缩回）</div>
                  <div className="settings-label-desc">
                    鼠标移出窗口多久之后自动收成宠物；正在打字或有未保存改动时不会收
                  </div>
                </div>
                <input
                  type="number"
                  min={0}
                  max={5000}
                  step={100}
                  value={petDraft.autoCollapseMs}
                  className="settings-number-input"
                  onChange={(e) =>
                    patchPet({
                      autoCollapseMs: Math.max(0, Math.min(5000, Number(e.target.value) || 0)),
                    })
                  }
                />
              </div>

              <button
                type="button"
                role="switch"
                aria-checked={petDraft.hideOnFullscreen}
                className="settings-row"
                onClick={() => patchPet({ hideOnFullscreen: !petDraft.hideOnFullscreen })}
              >
                <div className="settings-label">
                  <div className="settings-label-title">他人全屏时让路</div>
                  <div className="settings-label-desc">
                    检测到游戏 / 投屏 / 全屏播放时整只宠物隐身，退出全屏后自动回到原位
                  </div>
                </div>
                <div className={`settings-toggle ${petDraft.hideOnFullscreen ? "on" : "off"}`}>
                  <div className="settings-toggle-knob" />
                </div>
              </button>
            </>
          )}
        </div>
      </div>
    </>
  );
}
