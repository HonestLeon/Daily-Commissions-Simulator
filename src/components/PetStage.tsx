import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useNoteStore } from "../stores/noteStore";
import {
  PET_ACTIONS,
  actionByFile,
  computePetGates,
  hourMatches,
  isAmbient,
  petAssetsReady,
  pickAmbient,
  pickRotation,
  pickWeighted,
  type PetAction,
  type PetGate,
} from "./petAnimations";

// 位移超过这个数才算「拖拽」而不是「单击」。宠物窗只有 175×221，不区分就会在
// 用户想点开它时把整只宠物拖走（参考项目同样用 4px 阈值）。
const DRAG_THRESHOLD = 4;

// PET_ACTION_RANGES 的下标：4=安静(120–180s)、5=很安静(180–300s)。
// 这两档用户要的是「安静」，而素材是按标准节奏录的 10 秒循环，原速放会觉得手脚太快。
const QUIET_INDEX_MIN = 4;
const QUIET_RATE = 0.5;

// 宠物超过这么久没被碰过，就自己打个招呼（挥手 / 傲娇）刷存在感。
const LONELY_MS = 5 * 60_000;

// 金袋演完至少要隔这么久才允许再演一次：素材约 10 秒，中途重播等于把画面剪回第一帧
const COINS_COOLDOWN_MS = 8_000;

// Daily 动作是“每天一次”，不能放在组件 ref 里：便签态会卸载 PetStage，
// 每次缩回宠物 ref 都会重置，6–9 点就会反复重播刷牙。
let dailyPlayedOn = "";
function dayKey(): string {
  const d = new Date();
  return `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}`;
}

/** 常驻段在安静档以上放慢放；一次性动作保持原速（那是「演一遍」的手势，放慢会显得拖沓） */
function clipRate(action: PetAction, actionIndex: number): number {
  return isAmbient(action) && actionIndex >= QUIET_INDEX_MIN ? QUIET_RATE : 1;
}

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
  const coinsPending = useNoteStore((s) => s.coinsPending);
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
  const down = useRef<{ x: number; y: number } | null>(null);
  const dragging = useRef(false);
  // 当前在演的那段：onEnded 要靠它区分「演完回常驻」还是「常驻自己循环」
  const current = useRef<PetAction | null>(null);
  // 当前动作已经分配给哪一层。新动作还在加载时旧层的 ended 不能当成“演完了”
  const actionLayer = useRef<"a" | "b" | null>(null);
  // 最近一次被点击/拖动的时间戳：超过 LONELY_MS 没碰，就让宠物自己打个招呼刷存在感
  const lastTouch = useRef(Date.now());
  // 上一次轮换挑中的一次性动作：同一段不连着演两遍（见 nextIdle）
  const lastShot = useRef<string | null>(null);
  // 上一次演金袋的时间：见 COINS_COOLDOWN_MS
  const lastCoinsAt = useRef(0);

  /** 挑动作所需的事实：门控来自 Rust 侧算好的今日进度（唯一权威来源） */
  const facts = (): PetFacts => {
    const st = useNoteStore.getState();
    return { hour: new Date().getHours(), gates: computePetGates(st.progress) };
  };

  /**
   * 播放一段动作。`payCoins` 表示这段演的是「欠用户的一次金袋」：这笔账必须等画面
   * 真的开始播（play() resolve）才结。被 StrictMode 的首次挂载作废、或自动播放/解码
   * 失败时都不算演过——dev 下 StrictMode 会「挂载 → cleanup → 挂载」，在挂载时同步
   * 清账会让金袋在第一趟被作废的入场里花掉，第二趟只剩请安。
   */
  const play = useCallback((action: PetAction | null, opts?: { payCoins?: boolean }) => {
    if (!action) return;
    const gen = ++generation.current;
    const nextKey = layerRef.current === "a" ? "b" : "a";
    const el = nextKey === "a" ? vA.current : vB.current;
    const old = nextKey === "a" ? vB.current : vA.current;
    if (!el) return;
    // 只有「常驻画面」那几段（呼吸待机、夜里的小憩）才循环；哼歌 / 写代码 / 垂头叹气
    // 这类是演一遍的手势，循环起来等于每 10 秒重演一次，和换段间隔脱钩，看着像卡住。
    el.loop = isAmbient(action);
    current.current = action;
    actionLayer.current = nextKey;
    el.src = action.url;
    el.load();
    // load() 会把 playbackRate 复位，所以速率必须在它之后设
    el.playbackRate = clipRate(action, useNoteStore.getState().petActionIndex);
    const start = () => {
      if (gen !== generation.current) return; // 已被更新的播放作废
      if (watchdog.current) clearTimeout(watchdog.current);
      // 自动播放被拦/解码失败时保持当前画面，不弹空窗
      void el
        .play()
        .then(() => {
          // 确认出画才结金袋的账；这期间被新动作/卸载作废的，账留着下次入场再兑现
          if (gen === generation.current && opts?.payCoins) {
            useNoteStore.setState({ coinsPending: false });
          }
        })
        .catch(() => {});
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

  /** 下一次换段：区间内随机，不用固定周期（固定间隔会被用户摸出规律，显得机械） */
  const armTimer = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    const st = useNoteStore.getState();
    const min = Math.min(st.petActionMinSec, st.petActionMaxSec);
    const max = Math.max(st.petActionMinSec, st.petActionMaxSec);
    const delay = (min + Math.random() * (max - min)) * 1000;
    timer.current = setTimeout(() => nextIdleRef.current(), delay);
  }, []);

  /**
   * 下一次换段：时段事件（每天一次）优先，其次「好久没被理」的主动招呼，
   * 最后才按权重换一段非常驻画面。两次挑选之间由常驻段（呼吸 / 小憩）循环兜着。
   */
  const nextIdle = useCallback(() => {
    const f = facts();
    const today = dayKey();
    const daily = PET_ACTIONS.find(
      (a) => a.Pool === "Daily" && dailyPlayedOn !== today && hourMatches(a, f.hour)
    );
    if (daily) {
      dailyPlayedOn = today;
      play(daily); // 不循环，播完 onEnded 会接回常驻画面
    } else {
      const hi = Date.now() - lastTouch.current > LONELY_MS ? pickWeighted("Click", f) : null;
      if (hi) {
        lastTouch.current = Date.now(); // 打过一次就重新计时，别每段都刷存在感
        play(hi);
      } else {
        // 常驻段退出轮换后，深夜候选可能只剩「偷吃零食」一段：每个间隔都重嚼一次，
        // 就变成用户抱怨过的「不停做表情」。挑到和上次同一段就什么都不演，等于回到呼吸。
        const shot = pickRotation(f);
        if (shot && shot.File !== lastShot.current) {
          lastShot.current = shot.File;
          play(shot);
        }
      }
    }
    armTimer();
  }, [armTimer, play]);
  nextIdleRef.current = nextIdle;

  /**
   * 一次性动作（点击 / 拖拽 / 请安 / 刷牙 / 哼歌 / 写代码 / 叹气）演完回到常驻画面。
   * 这里故意不重排定时器：换段节奏只由「动作间隔」说了算。否则每演完一段就顺手挑下一段，
   * 用户设的 120–180 s 会被压成「素材长度 + 一点点」，安静档位照样不停做表情。
   */
  const onActionEnded = useCallback(
    (layer: "a" | "b") => {
      // 旧层被新动作盖住后可能还会补一个 ended；它不能打断正在演的新动作。
      // 新动作加载期间 layerRef 还指向旧层，所以还要比对 current 动作所在层。
      if (layer !== layerRef.current || layer !== actionLayer.current) return;
      const a = current.current;
      if (!a || isAmbient(a)) return; // 常驻段自己循环，压根到不了 ended
      play(pickAmbient(facts()));
    },
    [play]
  );

  /**
   * 换「动作间隔」档位时立刻改正在循环那段的速率：设置面板的契约是改完即生效，
   * 而常驻段是无限循环的，不主动改 rate 就得等下一次换段才看得到变化。
   */
  const petActionIndex = useNoteStore((s) => s.petActionIndex);
  useEffect(() => {
    const el = layerRef.current === "a" ? vA.current : vB.current;
    const a = current.current;
    if (!el || !a) return;
    el.playbackRate = clipRate(a, petActionIndex);
  }, [petActionIndex]);

  /**
   * 入场画面：欠着一次金袋（便签态刚勾掉过委托）就演金袋，否则照常请安。
   * 两者不能都演——请安排在前面会把刚出来的金袋直接盖掉，用户要的「你做完的事我看见了」
   * 就永远看不见。
   */
  const playEntry = useCallback(() => {
    const coins = useNoteStore.getState().coinsPending ? actionByFile("mood-coins") : null;
    // 同步占位，压掉同一拍里下面那条 effect 的重复补演；这里只占位不结账，
    // 账交给 play 在确认出画后清（否则被作废的这次入场会把账白白花掉）。
    if (coins) lastCoinsAt.current = Date.now();
    play(coins ?? actionByFile("greet-bow") ?? pickWeighted("Idle", facts()), {
      payCoins: !!coins,
    });
    armTimer();
  }, [play, armTimer]);

  useEffect(() => {
    if (!useVideo) return;
    // 画面不可见（启动即被托盘收走 / 全屏避让）时不要解码，也不要排动作；
    // 等可见性变 true 时由下面的 unpause 分支补一次入场。
    if (mediaPaused) return;
    // 入场一段（金袋或请安）是不循环的段，所以同时把换段定时器挂上兜底——
    // 万一这段播不到头（自动播放被拦等），宠物也仍会按节奏换动作
    playEntry();
    return () => {
      generation.current += 1;
      if (timer.current) clearTimeout(timer.current);
      if (watchdog.current) clearTimeout(watchdog.current);
      // 隐藏/卸载时必须显式暂停，否则解码会继续吃 CPU
      vA.current?.pause();
      vB.current?.pause();
    };
  }, [useVideo, playEntry]);

  // 隐藏启动时上面的挂载 effect 提前返回了，没有 cleanup；补一条独立的卸载清理
  useEffect(() => {
    return () => {
      generation.current += 1;
      if (timer.current) clearTimeout(timer.current);
      if (watchdog.current) clearTimeout(watchdog.current);
      vA.current?.pause();
      vB.current?.pause();
    };
  }, []);

  /**
   * 画面不可见（隐藏到托盘 / 避让全屏）时停掉解码与调度。挂载那趟交给上面那条 effect
   * （它负责入场一段），这里只处理真正的可见性翻转，否则会和入场动作抢画面。
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
    if (!current.current || useNoteStore.getState().coinsPending) {
      // 隐藏启动时挂载 effect 跳过了播放：第一次真正可见要补一次入场。
      // 或者欠着一次金袋（保存时宠物看不见）：也走入场，让金袋顶掉请安。
      playEntry();
      return; // playEntry 自己排好了换段
    }
    const live = layerRef.current === "a" ? vA.current : vB.current;
    void live?.play().catch(() => {}); // 被自动播放策略拦下就保持当前画面
    armTimer();
  }, [mediaPaused, playEntry, armTimer, play]);

  /**
   * 宠物在场时收到「又勾掉一条」的事件（保存比入场慢半拍就会走到这里）→ 当场补演金袋。
   * 8 秒内不重播：中途重播等于把画面剪回第一帧，看着像卡带；那种情况下账先欠着，
   * 下一次入场或下一次可见时兑现。
   */
  useEffect(() => {
    if (!coinsPending || mediaPaused || !useVideo) return;
    if (Date.now() - lastCoinsAt.current < COINS_COOLDOWN_MS) return;
    const coins = actionByFile("mood-coins");
    if (!coins) return;
    // 同样只占位不结账：账由 play 在确认出画后清（见 play 的 payCoins 注释）
    lastCoinsAt.current = Date.now();
    play(coins, { payCoins: true });
    armTimer(); // 插队演完要重排换段，否则原来排好的定时器会很快把金袋盖掉
  }, [coinsPending, mediaPaused, useVideo, play, armTimer]);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    down.current = { x: e.screenX, y: e.screenY };
    dragging.current = false;
    lastTouch.current = Date.now();
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
    // 单击只有「展开」这一条路，而展开没有门禁（拦的是缩回：未保存改动 / 正在输入），
    // 所以原来「展开被拦下才补个表情」根本走不到；打招呼改由 LONELY_MS 触发。
    await setForm("note");
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
            onEnded={() => onActionEnded("a")}
          />
          <video
            ref={vB}
            className={`pet-video${layer === "b" ? " is-active" : ""}`}
            muted
            playsInline
            preload="auto"
            onError={() => setBroken(true)}
            onEnded={() => onActionEnded("b")}
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
