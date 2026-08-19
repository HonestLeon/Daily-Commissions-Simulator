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

/** 标题栏置顶模式：pin=始终置顶；alarm=定时置顶提醒；normal=正常 */
export type PinMode = "pin" | "alarm" | "normal";

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
  settle_triple: boolean;
  settled_note_id: string | null;
  created_today: boolean;
}

export interface WeeklyDay {
  day: string;   // MMDD
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
