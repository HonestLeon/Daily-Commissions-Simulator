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

export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  download_url: string;
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
  days: WeeklyDay[];
}
