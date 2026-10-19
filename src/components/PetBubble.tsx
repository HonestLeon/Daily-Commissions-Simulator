import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getCurrentWindow,
  monitorFromPoint,
  PhysicalPosition,
  PhysicalSize,
} from "@tauri-apps/api/window";
import type { BubblePayload } from "../types";

/** 约 28 字/秒，跟参考项目一致：再快就读不过来，再慢就像卡了 */
const CHARS_PER_SEC = 28;
const EDGE_GAP = 8;

function clamp(v: number, min: number, max: number): number {
  return Math.min(Math.max(v, min), Math.max(min, max));
}

/**
 * 宠物说话的气泡窗（label = "bubble"，独立顶层小窗）。
 *
 * 为什么单开一个窗而不是画在主窗里：主窗在桌宠态必须等于精灵的可见包围盒，
 * 把气泡塞进去就得把窗口放大到「精灵 ∪ 气泡」，中间那块透明区在 Tauri 上点不穿
 * （没有逐像素命中），会变成一块挡点击的死区。气泡窗自己紧贴实测文字盒，死区最小。
 */
export default function PetBubble() {
  const [text, setText] = useState("");
  const boxRef = useRef<HTMLDivElement | null>(null);
  const anchorRef = useRef<BubblePayload["anchor"] | null>(null);
  // 单调版本号：上一条还没吐完就被新一条打断时，旧回调必须作废
  const version = useRef(0);
  const typeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const stopTimers = () => {
    if (typeTimer.current) clearTimeout(typeTimer.current);
    if (hideTimer.current) clearTimeout(hideTimer.current);
    typeTimer.current = null;
    hideTimer.current = null;
  };

  const hide = useCallback(() => {
    version.current += 1;
    stopTimers();
    setText("");
    getCurrentWindow()
      .hide()
      .catch(() => {});
  }, []);

  /** 按实测文字盒收紧窗口并定位：优先宠物上方，放不下翻到下方 */
  const refit = useCallback(async () => {
    const box = boxRef.current;
    const anchor = anchorRef.current;
    if (!box || !anchor || !text) return;
    const win = getCurrentWindow();
    const sf = await win
      .scaleFactor()
      .then((s) => s || 1)
      .catch(() => 1);
    const rect = box.getBoundingClientRect();
    const w = Math.max(1, Math.ceil(rect.width * sf));
    const h = Math.max(1, Math.ceil(rect.height * sf));
    // 用宠物所在的显示器工作区（多屏时不能拿主屏硬算）
    const wa = await monitorFromPoint(anchor.x + anchor.w / 2, anchor.y + anchor.h / 2)
      .then((m) =>
        m
          ? {
              x: m.workArea.position.x,
              y: m.workArea.position.y,
              w: m.workArea.size.width,
              h: m.workArea.size.height,
            }
          : null
      )
      .catch(() => null);
    let x = anchor.x + anchor.w / 2 - w / 2;
    let y = anchor.y - h - EDGE_GAP;
    if (wa && y < wa.y + EDGE_GAP) y = anchor.y + anchor.h + EDGE_GAP; // 上面挤不下就翻到下面
    if (wa) {
      x = clamp(x, wa.x + EDGE_GAP, wa.x + wa.w - w - EDGE_GAP);
      y = clamp(y, wa.y + EDGE_GAP, wa.y + wa.h - h - EDGE_GAP);
    }
    await win.setSize(new PhysicalSize(w, h)).catch(() => {});
    await win.setPosition(new PhysicalPosition(Math.round(x), Math.round(y))).catch(() => {});
  }, [text]);

  useEffect(() => {
    void refit();
  }, [refit]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let unlistenHide: (() => void) | undefined;
    let dead = false;

    const start = (payload: BubblePayload) => {
      const gen = ++version.current;
      stopTimers();
      anchorRef.current = payload.anchor;
      void getCurrentWindow()
        .show()
        .catch(() => {});
      // 按字素簇切，否则 emoji / 组合字会被撕成半个
      const seg = new Intl.Segmenter(undefined, { granularity: "grapheme" });
      const chars = Array.from(seg.segment(payload.text), (s) => s.segment);
      const stepMs = 1000 / CHARS_PER_SEC;
      let i = 0;
      const tick = () => {
        if (gen !== version.current) return;
        i += 1;
        setText(chars.slice(0, i).join(""));
        if (i < chars.length) typeTimer.current = setTimeout(tick, stepMs);
      };
      tick();
      // 停留时长至少要把打字时间算进去，否则长句子会被腰斩
      const showMs = Math.max(payload.ttlMs, chars.length * stepMs + 1500);
      hideTimer.current = setTimeout(() => {
        if (gen === version.current) hide();
      }, showMs);
    };

    listen<BubblePayload>("pet-bubble", ({ payload }) => start(payload))
      .then((fn) => {
        if (dead) fn();
        else unlisten = fn;
      })
      .catch(() => {}); // 非 Tauri 环境没有事件总线
    // 宠物让路（避让全屏、收进托盘）时话不能还挂在半空
    listen("pet-bubble-hide", () => hide())
      .then((fn) => {
        if (dead) fn();
        else unlistenHide = fn;
      })
      .catch(() => {});
    return () => {
      dead = true;
      unlisten?.();
      unlistenHide?.();
      stopTimers();
    };
  }, [hide]);

  if (!text) return null;

  return (
    <div className="pet-bubble-wrap" onPointerUp={hide} title="点击关闭">
      <div className="pet-bubble" ref={boxRef}>
        {text}
      </div>
    </div>
  );
}
