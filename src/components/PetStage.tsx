import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useNoteStore } from "../stores/noteStore";
import {
  PET_ACTIONS,
  actionByFile,
  computePetGates,
  hourMatches,
  petAssetsReady,
  pickWeighted,
  type PetAction,
  type PetGate,
} from "./petAnimations";

// 位移超过这个数才算「拖拽」而不是「单击」。宠物窗只有 175×221，不区分就会在
// 用户想点开它时把整只宠物拖走（参考项目同样用 4px 阈值）。
const DRAG_THRESHOLD = 4;

interface PetFacts {
  hour: number;
  gates: Set<PetGate>;
}

/**
 * 桌宠态的画面：双缓冲 <video> 播 α-WebM（VP9 + alpha，WebView2 原生支持）。
 *
 * 两层交替是因为直接换 src 会先清画面再出新帧，看上去就是「闪一下空白」；
 * 参考项目用常驻位图原地覆写来避免，我们等价地用「先把新段解到隐藏层，解出首帧
 * 再整层切换」，旧层停在最后一帧被盖住。
 *
 * 素材不全或解码失败时退回色块（pet-body），绝不留一只透明空窗。
 */
export default function PetStage() {
  const setForm = useNoteStore((s) => s.setForm);
  const mediaPaused = useNoteStore((s) => s.mediaPaused);
  // 解码失败（例如内嵌资源协议不满足 <video> 的取流方式）时自动退回色块，
  // 绝不让用户看到一只透明的空窗
  const [broken, setBroken] = useState(false);
  const useVideo = petAssetsReady() && !broken;
  const [layer, setLayer] = useState<"a" | "b">("a");
  const layerRef = useRef<"a" | "b">("a");
  const vA = useRef<HTMLVideoElement | null>(null);
  const vB = useRef<HTMLVideoElement | null>(null);
  // 单调版本号：任何打断源都自增，迟到的 loadeddata/play 回调一律作废
  const generation = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const watchdog = useRef<ReturnType<typeof setTimeout> | null>(null);
  const dailyDone = useRef(false);
  const down = useRef<{ x: number; y: number } | null>(null);
  const dragging = useRef(false);

  /** 挑动作所需的事实：门控来自 Rust 侧算好的今日进度（唯一权威来源） */
  const facts = (): PetFacts => {
    const st = useNoteStore.getState();
    return { hour: new Date().getHours(), gates: computePetGates(st.progress, st.settledAt) };
  };

  const play = useCallback((action: PetAction | null) => {
    if (!action) return;
    const gen = ++generation.current;
    const nextKey = layerRef.current === "a" ? "b" : "a";
    const el = nextKey === "a" ? vA.current : vB.current;
    const old = nextKey === "a" ? vB.current : vA.current;
    if (!el) return;
    // Idle 段循环播放：素材是一段十秒左右的表演，播完就静止的话，宠物在两次换段之间
    // （默认 45–90 秒）会看起来像一张死图。交互段（点击/拖拽/请安/刷牙）不循环，播完接回 Idle。
    el.loop = action.Pool === "Idle";
    el.src = action.url;
    el.load();
    const start = () => {
      if (gen !== generation.current) return; // 已被更新的播放作废
      if (watchdog.current) clearTimeout(watchdog.current);
      // 自动播放被拦/解码失败时保持当前画面，不弹空窗
      void el.play().catch(() => {});
      layerRef.current = nextKey;
      setLayer(nextKey);
      old?.pause();
    };
    if (el.readyState >= 2) {
      start();
    } else {
      el.addEventListener("loadeddata", start, { once: true });
      // 取不到首帧 = 这条播放路径不通（例如内嵌协议不满足视频取流方式），退色块
      if (watchdog.current) clearTimeout(watchdog.current);
      watchdog.current = setTimeout(() => {
        if (gen === generation.current && el.readyState < 2) setBroken(true);
      }, 2500);
    }
  }, []);

  // 定时器只负责「换下一段」，不负责让宠物动起来（动靠 Idle 段自身循环）。
  // 用 ref 转接是为了避开 nextIdle ↔ armTimer 的 useCallback 循环依赖。
  const nextIdleRef = useRef<() => void>(() => {});

  /** 下一次换段：区间内随机，不用固定周期（固定间隔会被用户摸出规律变成闹钟） */
  const armTimer = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    const st = useNoteStore.getState();
    const min = Math.min(st.petActionMinSec, st.petActionMaxSec);
    const max = Math.max(st.petActionMinSec, st.petActionMaxSec);
    const delay = (min + Math.random() * (max - min)) * 1000;
    timer.current = setTimeout(() => nextIdleRef.current(), delay);
  }, []);

  /** 接回「常驻画面」：时段事件（每天一次）优先，否则从 Idle 池挑一段循环播 */
  const nextIdle = useCallback(() => {
    const f = facts();
    const daily = PET_ACTIONS.find(
      (a) => a.Pool === "Daily" && !dailyDone.current && hourMatches(a, f.hour)
    );
    if (daily) {
      dailyDone.current = true;
      play(daily); // 不循环，播完 onEnded 会再接回 Idle
    } else {
      play(pickWeighted("Idle", f));
    }
    armTimer();
  }, [armTimer, play]);
  nextIdleRef.current = nextIdle;

  /** 交互段（点击 / 拖拽 / 请安 / 刷牙）播完接回常驻画面，不留静止的最后一帧 */
  const onActionEnded = useCallback(() => nextIdleRef.current(), []);

  useEffect(() => {
    if (!useVideo) return;
    // 每次进入桌宠态先请安一次；请安是不循环的段，所以同时把换段定时器挂上兜底——
    // 万一这段播不到头（自动播放被拦等），宠物也仍会按节奏换动作
    play(actionByFile("greet-bow") ?? pickWeighted("Idle", facts()));
    armTimer();
    return () => {
      generation.current += 1;
      if (timer.current) clearTimeout(timer.current);
      if (watchdog.current) clearTimeout(watchdog.current);
      // 隐藏/卸载时必须显式暂停，否则解码会继续吃 CPU
      vA.current?.pause();
      vB.current?.pause();
    };
  }, [useVideo, play, armTimer]);

  /**
   * 画面不可见（隐藏到托盘 / 避让全屏）时停掉解码与调度。挂载那趟交给上面那条 effect
   * （它负责请安），这里只处理真正的可见性翻转，否则会和入场动作抢画面。
   */
  const wasPaused = useRef(mediaPaused);
  useEffect(() => {
    const prev = wasPaused.current;
    wasPaused.current = mediaPaused;
    if (prev === mediaPaused) return;
    if (mediaPaused) {
      generation.current += 1; // 作废在途的 loadeddata / play 回调
      if (timer.current) clearTimeout(timer.current);
      if (watchdog.current) clearTimeout(watchdog.current);
      vA.current?.pause();
      vB.current?.pause();
      return;
    }
    const live = layerRef.current === "a" ? vA.current : vB.current;
    void live?.play().catch(() => {}); // 被自动播放策略拦下就保持当前画面
    armTimer();
  }, [mediaPaused, armTimer]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    down.current = { x: e.screenX, y: e.screenY };
    dragging.current = false;
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!down.current || dragging.current) return;
    const dx = Math.abs(e.screenX - down.current.x);
    const dy = Math.abs(e.screenY - down.current.y);
    if (dx + dy <= DRAG_THRESHOLD) return;
    dragging.current = true;
    play(actionByFile("drag-hold"));
    // 剩下的交给操作系统搬窗口，落点由 onMoved 的 debounce 记录（含贴边吸附）
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  };

  const onPointerUp = async () => {
    if (!down.current) return;
    const moved = dragging.current;
    down.current = null;
    dragging.current = false;
    if (moved) return; // OS 拖拽期间通常收不到这个事件，最坏是这次不展开，不会误展开
    const before = useNoteStore.getState().form;
    await setForm("note");
    // 展开被门禁拦下（有未保存改动 / 用户正在编辑器里操作）时给个反应，
    // 而不是点上去毫无动静
    if (useNoteStore.getState().form === before) {
      play(pickWeighted("Click", facts()));
    }
  };

  return (
    <div
      className="pet-stage"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      title="单击展开今日委托，拖拽移动"
    >
      {useVideo ? (
        <>
          <video
            ref={vA}
            className={`pet-video${layer === "a" ? " is-active" : ""}`}
            muted
            playsInline
            preload="auto"
            onError={() => setBroken(true)}
            onEnded={onActionEnded}
          />
          <video
            ref={vB}
            className={`pet-video${layer === "b" ? " is-active" : ""}`}
            muted
            playsInline
            preload="auto"
            onError={() => setBroken(true)}
            onEnded={onActionEnded}
          />
        </>
      ) : (
        <div className="pet-body">
          <div className="pet-face">
            <span className="pet-eye" />
            <span className="pet-eye" />
          </div>
        </div>
      )}
    </div>
  );
}
