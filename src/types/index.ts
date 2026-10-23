export interface NoteMeta {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  title_edited?: boolean;
  settled?: boolean;
  triple_complete?: boolean;
}

export type ViewMode = "edit" | "preview" | "split";

/** 颜色主题：light=默认浅色；sky/sakura/green=彩色主题 */
export type AppTheme = "light" | "sky" | "sakura" | "green";

/** 窗口形态：pet=桌宠态（紧贴精灵包围盒）；note=便签态（现有完整功能） */
export type Form = "pet" | "note";

/** Rust `get_ui_state` 的返回：形态、桌宠开关与两套几何（均为物理像素） */
export interface UiState {
  pet_enabled: boolean;
  form: Form;
  pet_scale: number;
  /** 动作间隔档位下标（Rust 侧 PET_ACTION_RANGES） */
  action_index: number;
  action_min_sec: number;
  action_max_sec: number;
  auto_collapse_ms: number;
  hide_on_fullscreen: boolean;
  pet_x: number | null;
  pet_y: number | null;
  note_x: number | null;
  note_y: number | null;
  note_w: number | null;
  note_h: number | null;
}

/** 设置面板「桌宠」分区要写的五项；Rust `set_pet_config` 的入参（invoke 用 camelCase） */
export interface PetConfig {
  petEnabled: boolean;
  scale: number;
  actionIndex: number;
  autoCollapseMs: number;
  hideOnFullscreen: boolean;
}

/** Rust `get_today_progress` 的返回：桌宠动作/气泡要用当天的事实 */
export interface TodayProgress {
  modules_done: number;
  triple: boolean;
  /** 三个结算模块里是否至少有一条非空委托（全空的委托不该被夸） */
  has_tasks: boolean;
  /** 三个结算模块里已勾选的非空条数；Rust 用它算出增量并广播 task-completed */
  checked_count: number;
  /** 学习/工作、生活/其他 两个模块里是否填了内容 */
  tomorrow_filled: boolean;
  note_found: boolean;
}

/** 主窗发给 bubble 窗的一次说话请求（纯 JS↔JS 事件载荷，用 camelCase） */
export interface BubblePayload {
  text: string;
  /** 停留毫秒；至少要够吐完字，0 = 直到下一条 */
  ttlMs: number;
  /** 宠物窗的物理几何，气泡按它定位 */
  anchor: { x: number; y: number; w: number; h: number };
}

export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  /** 安装版安装包下载地址（setup exe） */
  download_url: string;
  /** 免安装版（绿色版）exe 下载地址 */
  portable_download_url: string;
}

export interface InitDailyResult {
  yuan_shi: number;
  settle_reward: number;
  /** 结算后的摩拉余额（以后端为准） */
  mora: number;
  /** 本次结算获得的摩拉 */
  settle_mora: number;
  settle_triple: boolean;
  settled_note_id: string | null;
  created_today: boolean;
}

export interface WeeklyDay {
  day: string; // MMDD
  triple: boolean;
}

export interface WeeklyResult {
  count: number;
  eligible: boolean;
  reward: number;
  streak: number;
  /** 结算后的权威原石余额（以后端为准） */
  yuan_shi: number;
  days: WeeklyDay[];
}
