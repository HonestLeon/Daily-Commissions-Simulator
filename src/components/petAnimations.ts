import idleBreath from "../assets/pet/idle-breath.webm";
import idleSleep from "../assets/pet/idle-sleep.webm";
import idleHum from "../assets/pet/idle-hum.webm";
import clickWave from "../assets/pet/click-wave.webm";
import clickPout from "../assets/pet/click-pout.webm";
import dragHold from "../assets/pet/drag-hold.webm";
import greetBow from "../assets/pet/greet-bow.webm";
import dailyBrush from "../assets/pet/daily-brush.webm";
import moodSulk from "../assets/pet/mood-sulk.webm";
import moodCoins from "../assets/pet/mood-coins.webm";
import moodWork from "../assets/pet/mood-work.webm";
import moodSnack from "../assets/pet/mood-snack.webm";
import manifest from "../assets/pet/pet_actions.json";
import type { TodayProgress } from "../types";

/**
 * 动作池。Idle 用于无人打断时的轮换；Click/Drag/Greeting 由交互触发；
 * Daily 是时段事件；Notify 留给数据事件（结算完成、明日委托没填）。
 */
export type PetPool = "Idle" | "Click" | "Drag" | "Greeting" | "Daily" | "Notify";

/** 插入门控：只有 Rust 侧算出的事实满足时才参与随机 */
export type PetGate = "incomplete" | "settled" | "empty_note" | "triple";

export interface PetActionRule {
  /** 中文动作名，日志与气泡文案用 */
  Id: string;
  /** 素材的 ASCII 文件名（不含扩展名），必须能在下面那张表里查到 */
  File: string;
  Pool: PetPool;
  /** 权重 0 表示不参与随机 */
  Weight: number;
  HourFrom?: number;
  HourTo?: number;
  Gate?: PetGate;
}

export interface PetAction extends PetActionRule {
  url: string;
}

// 逐个静态 import 而不是 import.meta.glob：清单里写了不存在的素材会在
// `npm run build` 就直接失败，而不是等用户看到一只空白的宠物。
const URLS: Record<string, string> = {
  "idle-breath": idleBreath,
  "idle-sleep": idleSleep,
  "idle-hum": idleHum,
  "click-wave": clickWave,
  "click-pout": clickPout,
  "drag-hold": dragHold,
  "greet-bow": greetBow,
  "daily-brush": dailyBrush,
  "mood-sulk": moodSulk,
  "mood-coins": moodCoins,
  "mood-work": moodWork,
  "mood-snack": moodSnack,
};

export const PET_ACTIONS: PetAction[] = (manifest as PetActionRule[]).flatMap((r) => {
  const url = URLS[r.File];
  if (!url) {
    console.error(`桌宠动作清单引用了未知素材：${r.File}`);
    return [];
  }
  return [{ ...r, url }];
});

/** 素材是否可用；不可用时 PetStage 退回色块，而不是留一只空窗 */
export function petAssetsReady(): boolean {
  return PET_ACTIONS.length > 0;
}

export function actionByFile(file: string): PetAction | null {
  return PET_ACTIONS.find((a) => a.File === file) ?? null;
}

/** 时段判定。23→8 这种跨午夜的区间按「晚于起 或 早于止」命中 */
export function hourMatches(rule: PetActionRule, hour: number): boolean {
  if (rule.HourFrom == null || rule.HourTo == null) return true;
  const { HourFrom: from, HourTo: to } = rule;
  return from <= to ? hour >= from && hour < to : hour >= from || hour < to;
}

/**
 * 由当天的事实算出「哪些门控成立」。门控全部来自 Rust（结算模块名、勾选判定都只有
 * 一份权威实现），前端不再自己数一遍。
 */
export function computePetGates(
  progress: TodayProgress | null,
  settledAt: number | null
): Set<PetGate> {
  const gates = new Set<PetGate>();
  if (progress?.note_found) {
    if (progress.triple) gates.add("triple");
    if (progress.has_tasks && !progress.triple) gates.add("incomplete");
    if (!progress.tomorrow_filled) gates.add("empty_note");
  }
  // 「刚结算」只在三分钟内有效，免得宠物整天挂着金袋子
  if (settledAt != null && Date.now() - settledAt < 3 * 60_000) gates.add("settled");
  return gates;
}

/** 按池 + 权重随机挑一个动作；没有候选就返回 null（调用方保持当前画面） */
export function pickWeighted(
  pool: PetPool,
  facts: { hour: number; gates: Set<PetGate> },
  random: () => number = Math.random
): PetAction | null {
  const candidates = PET_ACTIONS.filter(
    (a) =>
      a.Pool === pool &&
      a.Weight > 0 &&
      hourMatches(a, facts.hour) &&
      (!a.Gate || facts.gates.has(a.Gate))
  );
  if (candidates.length === 0) return null;
  const total = candidates.reduce((sum, a) => sum + a.Weight, 0);
  let roll = random() * total;
  for (const a of candidates) {
    roll -= a.Weight;
    if (roll <= 0) return a;
  }
  return candidates[candidates.length - 1];
}
