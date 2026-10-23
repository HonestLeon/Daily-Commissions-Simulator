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
 * 动作池。Idle 用于无人打断时的轮换（常驻的呼吸/小憩除外，它们不参与换段）；
 * Click/Drag/Greeting 由交互触发；Daily 是时段事件；Notify 是数据状态反馈——
 * 偷吃零食按权重混在常规轮换里，金袋权重 0，只由「有新委托完成」这个事件单独触发。
 */
export type PetPool = "Idle" | "Click" | "Drag" | "Greeting" | "Daily" | "Notify";

/** 插入门控：只有 Rust 侧算出的事实满足时才参与随机 */
export type PetGate = "incomplete" | "triple";

export interface PetActionRule {
  /** 中文动作名，日志与气泡文案用 */
  Id: string;
  /** 素材的 ASCII 文件名（不含扩展名），必须能在下面那张表里查到 */
  File: string;
  Pool: PetPool;
  /** 权重 0 表示不参与随机轮换（只由事件点名，例如金袋） */
  Weight: number;
  HourFrom?: number;
  HourTo?: number;
  Gate?: PetGate;
  /**
   * Idle 段默认循环播（素材只有十秒左右，不循环的话宠物在两次换段之间像张死图）。
   * 但「垂头叹气」这类情绪动作循环起来就是宠物卡住了，标 Loop:false 让它只演一遍。
   */
  Loop?: boolean;
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
export function computePetGates(progress: TodayProgress | null): Set<PetGate> {
  const gates = new Set<PetGate>();
  if (progress?.note_found) {
    if (progress.triple) gates.add("triple");
    if (progress.has_tasks && !progress.triple) gates.add("incomplete");
  }
  return gates;
}

/** 按池（可多个）+ 权重随机挑一个动作；没有候选就返回 null（调用方保持当前画面） */
export function pickWeighted(
  pool: PetPool | PetPool[],
  facts: { hour: number; gates: Set<PetGate> },
  random: () => number = Math.random,
  where?: (a: PetAction) => boolean
): PetAction | null {
  const pools = Array.isArray(pool) ? pool : [pool];
  const candidates = PET_ACTIONS.filter(
    (a) =>
      pools.includes(a.Pool) &&
      a.Weight > 0 &&
      hourMatches(a, facts.hour) &&
      (!a.Gate || facts.gates.has(a.Gate)) &&
      (!where || where(a))
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

/**
 * 常驻画面：只有「Idle 且没被标 Loop:false」的段才允许循环。
 * 素材清一色 10 秒一段，循环一段「哼歌 / 写代码」就等于每 10 秒把同一个手势重演一遍，
 * 和用户设的 120–180 s 换段间隔完全脱钩——看起来就是「安静档位下还在不停做表情」。
 */
export const isAmbient = (a: PetAction) => a.Pool === "Idle" && a.Loop !== false;

/** 挑一段常驻画面（呼吸待机 / 夜里的小憩），一次性动作演完就回到这里 */
export function pickAmbient(
  facts: { hour: number; gates: Set<PetGate> },
  random: () => number = Math.random
): PetAction | null {
  return pickWeighted("Idle", facts, random, isAmbient);
}

/**
 * 常规轮换挑一段：常驻段不参与（呼吸 / 小憩是「没换」的默认画面，混进轮换就等于
 * 每隔两分钟重播一次同一个呼吸），金袋这类 Weight:0 的也会被权重过滤掉。
 */
export function pickRotation(
  facts: { hour: number; gates: Set<PetGate> },
  random: () => number = Math.random
): PetAction | null {
  return pickWeighted(["Idle", "Notify"], facts, random, (a) => !isAmbient(a));
}
