use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use chrono::{Local, Datelike};
use tauri::Manager;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

// ── Daily commission ──

/// Fixed structure of a daily commission note. The three settled modules are
/// 必做 / 学习·工作 / 生活·其他; 随手小记 is a free-form area, not settled.
/// Each module defaults to a single empty item; inheritance fills them in.
const DAILY_CONTENT: &str = "### 每日委托\n\n#### 必做\n\n- [ ] \n\n#### 学习/工作\n\n- [ ] \n\n#### 生活/其他\n\n- [ ] \n\n#### 随手小记\n\n- \n";

/// Modules that participate in settlement (all checkboxes checked = complete;
/// an empty module counts as complete).
const SETTLE_MODULES: [&str; 3] = ["必做", "学习/工作", "生活/其他"];

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub title_edited: bool,
    #[serde(default)]
    pub settled: bool,          // 是否已结算（防重复结算）
    #[serde(default)]
    pub triple_complete: bool,  // 是否三全完成（砺行修远周统计用）
}

fn notes_dir(app: &tauri::AppHandle) -> PathBuf {
    let path = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("notes");
    fs::create_dir_all(&path).ok();
    path
}

fn index_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("notes_index.json")
}

fn read_index(app: &tauri::AppHandle) -> Vec<NoteMeta> {
    let path = index_path(app);
    if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        vec![]
    }
}

fn write_index(app: &tauri::AppHandle, notes: &[NoteMeta]) {
    let path = index_path(app);
    let data = serde_json::to_string_pretty(notes).unwrap_or_default();
    fs::write(&path, data).ok();
}

fn load_note_internal(app: &tauri::AppHandle, id: &str) -> Result<String, String> {
    let md_path = notes_dir(app).join(format!("{}.md", id));
    let content = fs::read_to_string(&md_path).map_err(|e| e.to_string())?;
    // 旧数据可能存在无尾空格的空复选框，读出时规范化以保证渲染
    Ok(ensure_task_trailing_space(&content))
}

fn create_note_with_content(
    app: &tauri::AppHandle,
    title: String,
    content: String,
) -> Result<NoteMeta, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = timestamp_now();
    let note = NoteMeta {
        id: id.clone(),
        title: title.trim().chars().take(100).collect(),
        created_at: now.clone(),
        updated_at: now,
        title_edited: false,
        settled: false,
        triple_complete: false,
    };
    let md_path = notes_dir(app).join(format!("{}.md", &id));
    fs::write(&md_path, content).map_err(|e| e.to_string())?;
    let mut notes = read_index(app);
    notes.push(note.clone());
    write_index(app, &notes);
    Ok(note)
}

// ── Date helpers ──

fn timestamp_now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn today_ymd() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// "2026-08-16T..." -> "0816"
fn format_daily_title(iso: &str) -> String {
    if iso.len() >= 10 {
        format!("{}{}", &iso[5..7], &iso[8..10])
    } else {
        String::new()
    }
}

fn is_today(iso: &str) -> bool {
    iso.len() >= 10 && &iso[..10] == today_ymd()
}

fn parse_date(iso: &str) -> Option<chrono::NaiveDate> {
    if iso.len() < 10 {
        return None;
    }
    chrono::NaiveDate::parse_from_str(&iso[..10], "%Y-%m-%d").ok()
}

/// Monday of the current natural week.
fn monday_of_current_week() -> chrono::NaiveDate {
    let today = Local::now().date_naive();
    let dow = today.weekday().num_days_from_monday();
    today - chrono::Duration::days(dow as i64)
}

// ── Settlement / 原石 ──

/// 提取复选框条目（`- [ ]` / `- [x]` / `* ` 变体）标记之后的文本；
/// 非复选框行返回 None。
fn checkbox_tail(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let t = if let Some(r) = t.strip_prefix("- [") {
        r
    } else if let Some(r) = t.strip_prefix("* [") {
        r
    } else {
        return None;
    };
    let t = if let Some(r) = t.strip_prefix("x]") {
        r
    } else if let Some(r) = t.strip_prefix("X]") {
        r
    } else if let Some(r) = t.strip_prefix(" ]") {
        r
    } else if let Some(r) = t.strip_prefix(']') {
        r
    } else {
        return None;
    };
    Some(t)
}

/// 空白项（空委托）：复选框标记后仅空白（含已勾选的空白项）。
/// 结算、周统计、继承均忽略此类条目。
fn is_blank_item(line: &str) -> bool {
    checkbox_tail(line)
        .map(|t| t.trim().is_empty())
        .unwrap_or(false)
}

/// 为独占一行且标记后无内容的任务复选框补一个尾空格：
/// Milkdown 需要 `- [ ] `（尾空格）才能把空项渲染为复选框。
/// 有内容的行原样保留。
fn ensure_task_trailing_space(content: &str) -> String {
    let mut out = String::with_capacity(content.len() + 8);
    for line in content.lines() {
        let trimmed = line.trim_end();
        // 仅处理形如 "- [ ]" / "- [x]"（标记后无任何内容）的行
        let is_bare_task = ["- [ ]", "- [x]", "- [X]", "* [ ]", "* [x]", "* [X]"]
            .iter()
            .any(|m| trimmed == *m);
        if is_bare_task {
            out.push_str(trimmed);
            out.push(' ');
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    // 保留原内容末尾无换行的形态
    if !content.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

/// A module is complete only when it has at least one non-blank checkbox and
/// all of its non-blank checkboxes are checked. A module with no non-blank
/// checkboxes (empty / blank-only) is NOT complete — empty commissions don't
/// count toward fulfillment, so users are encouraged to fill in tasks.
fn section_complete(section: &str) -> bool {
    let boxes: Vec<&str> = section
        .lines()
        .map(str::trim_start)
        .filter(|l| {
            (l.starts_with("- [") || l.starts_with("* [")) && !is_blank_item(l)
        })
        .collect();
    if boxes.is_empty() {
        return false;
    }
    boxes.iter().all(|l| {
        l.starts_with("- [x]")
            || l.starts_with("- [X]")
            || l.starts_with("* [x]")
            || l.starts_with("* [X]")
    })
}

/// Compute the settlement reward for a note's parsed sections.
/// reward = completed * 10 + (triple ? 20 : 0), i.e. 10/20/50.
fn settle_reward(sections: &std::collections::HashMap<String, String>) -> (u32, bool) {
    let completed = SETTLE_MODULES
        .iter()
        .filter(|m| sections.get(**m).map(|s| section_complete(s)).unwrap_or(false))
        .count();
    let triple = completed == 3;
    let reward = (completed as u32) * 10 + if triple { 20 } else { 0 };
    (reward, triple)
}

/// 摩拉：三个结算模块内「已勾选且非空」的任务条数 × 100。
/// 未勾选、空白项、引用块（明日委托）均不计入。
fn mora_reward(sections: &std::collections::HashMap<String, String>) -> u32 {
    let done = SETTLE_MODULES
        .iter()
        .filter_map(|m| sections.get(*m))
        .flat_map(|s| s.lines().map(str::trim_start))
        .filter(|l| {
            !is_blank_item(l)
                && (l.starts_with("- [x]")
                    || l.starts_with("- [X]")
                    || l.starts_with("* [x]")
                    || l.starts_with("* [X]"))
        })
        .count();
    (done as u32) * 100
}

struct SettleOutcome {
    id: String,
    reward: u32,
    mora: u32,
    triple: bool,
}

/// Settle the most recent, not-today, not-yet-settled daily commission.
/// Adds the reward to the 原石 balance and marks it settled.
fn settle_previous_note(app: &tauri::AppHandle) -> Option<SettleOutcome> {
    let mut notes = read_index(app);

    // Most recent, not-today, not-yet-settled commission (by updated_at).
    let target_idx = notes
        .iter()
        .enumerate()
        .filter(|(_, n)| !is_today(&n.created_at) && !n.settled)
        .max_by(|a, b| a.1.updated_at.cmp(&b.1.updated_at))
        .map(|(i, _)| i)?;

    let target_id = notes[target_idx].id.clone();
    let content = fs::read_to_string(notes_dir(app).join(format!("{}.md", &target_id))).ok()?;
    let sections = parse_md_sections(&content);
    let (reward, triple) = settle_reward(&sections);
    let mora = mora_reward(&sections);

    notes[target_idx].settled = true;
    notes[target_idx].triple_complete = triple;
    write_index(app, &notes);

    let mut config = load_config(app);
    config.yuan_shi += reward;
    config.mora += mora;
    save_config(app, &config);

    Some(SettleOutcome {
        id: target_id,
        reward,
        mora,
        triple,
    })
}

// ── Auto-create today's commission ──

/// Build today's content by inheriting from a previous commission: 必做 carries
/// every item (reset to unchecked), other settled modules carry only unchecked
/// items, 明日委托（引用块 `> ...`）转为普通任务继承到对应栏, and 随手小记
/// stays blank.
fn inherit_from_prev(prev: &str) -> String {
    let template = DAILY_CONTENT;
    let prev_sections = parse_md_sections(prev);
    let mut result = String::new();
    let mut in_section: Option<&str> = None;

    for line in template.lines() {
        if line.starts_with("#### ") {
            in_section = Some(line.trim_start_matches("#### ").trim());
            result.push_str(line);
            result.push('\n');
            result.push('\n');

            if let Some(sec_name) = in_section {
                // 随手小记 is free-form — never inherit; the template's own
                // placeholder lines below will be written as-is.
                if sec_name != "随手小记" {
                    let mut items: Vec<String> = Vec::new();
                    if let Some(prev_sec) = prev_sections.get(sec_name) {
                        let is_checkbox = |l: &&str| -> bool {
                            let t = l.trim_start();
                            t.starts_with("- [") || t.starts_with("* [")
                        };
                        let is_unchecked = |l: &&str| -> bool {
                            let t = l.trim_start();
                            t.starts_with("- [ ]") || t.starts_with("* [ ]")
                        };
                        // 普通待办：必做全量继承并重置勾选，其余只继承未完成项；
                        // 空白项（空委托）一律不继承
                        if sec_name == "必做" {
                            for item in prev_sec
                                .lines()
                                .filter(|l| is_checkbox(l) && !is_blank_item(l))
                            {
                                items.push(reset_checkbox(item));
                            }
                        } else {
                            for item in prev_sec
                                .lines()
                                .filter(|l| is_unchecked(l) && !is_blank_item(l))
                            {
                                items.push(item.to_string());
                            }
                        }
                        // 明日委托（引用块 `> text`）：在新一天转为普通任务 `- [ ] text`
                        for pline in prev_sec.lines() {
                            if let Some(text) = pline.trim_start().strip_prefix('>') {
                                let text = text.trim_start();
                                if !text.trim().is_empty() {
                                    items.push(format!("- [ ] {}", text.trim()));
                                }
                            }
                        }
                    }
                    if items.is_empty() {
                        items.push("- [ ] ".to_string());
                    }
                    for item in items {
                        result.push_str(&item);
                        result.push('\n');
                    }
                    continue;
                }
            }
            // Heading was written above; do not fall through and write it again.
            continue;
        }

        // Skip the template's blank placeholder checkboxes.
        let trimmed = line.trim_start();
        if trimmed == "- [ ]" || trimmed == "- [ ] " || trimmed == "* [ ]" || trimmed == "* [ ] " {
            continue;
        }
        // Keep heading/blank lines that aren't checkbox placeholders.
        if in_section.is_none() || !line.trim_start().starts_with("- [") {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

/// Find the most recent daily commission and build today's content from it.
fn inherit_daily(app: &tauri::AppHandle) -> String {
    let notes = read_index(app);
    let mut sorted: Vec<_> = notes.iter().collect();
    sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let prev_content = sorted.iter().find_map(|n| {
        let content = fs::read_to_string(notes_dir(app).join(format!("{}.md", &n.id))).ok()?;
        if content.contains("### 每日委托") {
            Some(content)
        } else {
            None
        }
    });

    match prev_content {
        Some(prev) => inherit_from_prev(&prev),
        None => DAILY_CONTENT.to_string(),
    }
}

/// Reset a checkbox line to unchecked, preserving the marker style (- or *).
fn reset_checkbox(line: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let result = if let Some(pos) = trimmed.find("[x]").or_else(|| trimmed.find("[X]")) {
        let mut s = trimmed.to_string();
        s.replace_range(pos..pos + 3, "[ ]");
        s
    } else {
        trimmed.to_string()
    };
    format!("{}{}", indent, result)
}

/// Parse markdown into sections keyed by #### heading name.
fn parse_md_sections(content: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut current_section = String::new();
    let mut current_name = String::new();

    for line in content.lines() {
        if line.starts_with("#### ") {
            if !current_name.is_empty() {
                map.insert(current_name.clone(), current_section.trim().to_string());
            }
            current_name = line.trim_start_matches("#### ").trim().to_string();
            current_section = String::new();
        } else if !current_name.is_empty() {
            current_section.push_str(line);
            current_section.push('\n');
        }
    }
    if !current_name.is_empty() {
        map.insert(current_name, current_section.trim().to_string());
    }
    map
}

/// Create today's commission if none exists yet. Returns the new note, or None
/// if today's commission already exists.
fn auto_create_daily(app: &tauri::AppHandle) -> Option<NoteMeta> {
    let notes = read_index(app);
    if notes.iter().any(|n| is_today(&n.created_at)) {
        return None;
    }
    let content = inherit_daily(app);
    let title = format_daily_title(&today_ymd());
    create_note_with_content(app, title, content).ok()
}

#[derive(Serialize)]
struct InitDailyResult {
    yuan_shi: u32,
    settle_reward: u32,
    mora: u32,
    settle_mora: u32,
    settle_triple: bool,
    settled_note_id: Option<String>,
    created_today: bool,
}

/// Called by the frontend at startup: settle the previous commission, then
/// auto-create today's commission if needed. Returns the outcome for display.
#[tauri::command]
fn init_daily(app: tauri::AppHandle) -> InitDailyResult {
    let settled = settle_previous_note(&app);
    let created = auto_create_daily(&app);
    let config = load_config(&app);
    InitDailyResult {
        yuan_shi: config.yuan_shi,
        settle_reward: settled.as_ref().map(|s| s.reward).unwrap_or(0),
        mora: config.mora,
        settle_mora: settled.as_ref().map(|s| s.mora).unwrap_or(0),
        settle_triple: settled.as_ref().map(|s| s.triple).unwrap_or(false),
        settled_note_id: settled.map(|s| s.id),
        created_today: created.is_some(),
    }
}

// ── 砺行修远 (weekly challenge) ──

#[derive(Serialize)]
struct WeeklyDay {
    day: String,   // MMDD
    triple: bool,
}

#[derive(Serialize)]
struct WeeklyResult {
    count: u32,        // 本周（周一~今天）三全完成天数
    eligible: bool,    // >= 5 天
    reward: u32,       // 达标：100*(streak+1)；未达标：0
    streak: u32,       // 更新后的连续达标周数
    yuan_shi: u32,     // 结算后的权威原石余额（以后端为准，前端不再本地累加）
    days: Vec<WeeklyDay>,
}

/// 周结算逻辑（纯函数，可测试）。
/// 返回 (是否达标, 应发奖励, 结算后的 streak)。
/// - `already = true`：本周已结算过 → 不再发奖，也不改动 streak（幂等，仅查看）。
/// - `already = false` 且达标：发奖 100*(streak+1)，连续周数 +1（满 7 周清零）。
/// - `already = false` 且未达标：连续周数清零，不发奖。
fn weekly_settle_logic(count: u32, streak: u32, already: bool) -> (bool, u32, u32) {
    let eligible = count >= 5;
    if already {
        (eligible, 0, streak)
    } else if eligible {
        let mut new_streak = streak + 1;
        if new_streak == 7 {
            new_streak = 0;
        }
        (true, 100 * (streak + 1), new_streak)
    } else {
        (false, 0, 0)
    }
}

#[tauri::command]
fn check_weekly(app: tauri::AppHandle) -> WeeklyResult {
    let monday = monday_of_current_week();
    let today = Local::now().date_naive();
    let notes = read_index(&app);

    let mut days: Vec<WeeklyDay> = Vec::new();
    let mut count: u32 = 0;

    let mut week_notes: Vec<(&NoteMeta, bool)> = Vec::new();
    for n in &notes {
        if let Some(d) = parse_date(&n.created_at) {
            if d >= monday && d <= today {
                let triple = if n.settled {
                    n.triple_complete
                } else {
                    // Today's note isn't settled yet — evaluate live.
                    fs::read_to_string(notes_dir(&app).join(format!("{}.md", &n.id)))
                        .ok()
                        .map(|c| settle_reward(&parse_md_sections(&c)).1)
                        .unwrap_or(false)
                };
                week_notes.push((n, triple));
            }
        }
    }
    week_notes.sort_by(|a, b| a.0.created_at.cmp(&b.0.created_at));

    for (n, triple) in week_notes {
        let d = parse_date(&n.created_at).unwrap();
        days.push(WeeklyDay {
            day: format!("{:02}{:02}", d.month(), d.day()),
            triple,
        });
        if triple {
            count += 1;
        }
    }

    let monday_str = monday_of_current_week().format("%Y-%m-%d").to_string();
    let mut config = load_config(&app);
    let already = config.weekly_settled_week == monday_str;
    let (eligible, reward, new_streak) = weekly_settle_logic(count, config.streak, already);
    if !already {
        config.streak = new_streak;
        config.yuan_shi += reward;
        // 方案 B：只有“达标发奖”才锁定本周（防止同周重复发奖）；
        // 未达标只清零连续周数、不锁定，本周后续补达标仍可获得奖励。
        if eligible {
            config.weekly_settled_week = monday_str;
        }
        save_config(&app, &config);
    }

    WeeklyResult {
        count,
        eligible,
        reward,
        streak: config.streak,
        yuan_shi: config.yuan_shi,
        days,
    }
}

// ── Note commands ──

#[tauri::command]
fn list_notes(app: tauri::AppHandle) -> Vec<NoteMeta> {
    read_index(&app)
}

#[tauri::command]
fn delete_note(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let md_path = notes_dir(&app).join(format!("{}.md", &id));
    fs::remove_file(&md_path).ok();
    let mut notes = read_index(&app);
    notes.retain(|n| n.id != id);
    write_index(&app, &notes);
    Ok(())
}

#[tauri::command]
fn save_note(app: tauri::AppHandle, id: String, content: String) -> Result<(), String> {
    let md_path = notes_dir(&app).join(format!("{}.md", &id));
    // 空复选框补尾空格，保证保存/重开后仍能渲染
    let content = ensure_task_trailing_space(&content);

    // 内容未变化则跳过：编辑器加载时 markdownUpdated 会回传一次原内容，
    // 若照样写入会刷新 updated_at、并把每日委托标题改成当天日期。
    if fs::read_to_string(&md_path)
        .map(|existing| existing == content)
        .unwrap_or(false)
    {
        return Ok(());
    }

    fs::write(&md_path, &content).map_err(|e| e.to_string())?;

    let mut notes = read_index(&app);
    if let Some(note) = notes.iter_mut().find(|n| n.id == id) {
        note.updated_at = timestamp_now();
        // Title reflects the commission's own day (created_at), never the
        // editing time — editing yesterday's note must not retitle it.
        if !note.title_edited {
            note.title = format_daily_title(&note.created_at);
        }
    }
    write_index(&app, &notes);
    Ok(())
}

#[tauri::command]
fn rename_note(app: tauri::AppHandle, id: String, new_title: String) -> Result<(), String> {
    let mut notes = read_index(&app);
    if let Some(note) = notes.iter_mut().find(|n| n.id == id) {
        note.title = new_title.trim().chars().take(100).collect();
        note.updated_at = timestamp_now();
        note.title_edited = true;
    }
    write_index(&app, &notes);
    Ok(())
}

#[tauri::command]
fn load_note(app: tauri::AppHandle, id: String) -> Result<String, String> {
    load_note_internal(&app, &id)
}

// ── App config ──

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppConfig {
    close_to_tray: bool,
    opacity: f32,
    #[serde(default)]
    yuan_shi: u32,
    #[serde(default)]
    mora: u32,
    #[serde(default)]
    streak: u32,
    /// 已结算的自然周（周一 YYYY-MM-DD）：保证周奖励每自然周只结算一次
    #[serde(default)]
    weekly_settled_week: String,
    /// 闹钟模式：每隔 alarm_interval 秒弹出置顶提醒（默认 10 分钟）
    #[serde(default)]
    alarm_interval: u64,
    /// 闹钟模式：置顶显示的时长（秒，最长 60）
    #[serde(default)]
    alarm_duration: u64,
    /// 是否启用整窗背景图皮肤（仅浅色模式生效）
    #[serde(default)]
    skin_enabled: bool,
    /// 界面颜色主题（light / sky / sakura / green，浅色底深色字）
    #[serde(default)]
    theme: String,
    /// 桌宠模式总开关。为 false 时行为与桌宠化之前完全一致（回滚保险）
    #[serde(default)]
    pet_enabled: bool,
    /// 上次退出时的窗口形态：pet=桌宠态，note=便签态
    #[serde(default)]
    form: String,
    /// 桌宠大小缩放，见 normalize_pet_config（上限刻意低于参考项目的 2.0）
    #[serde(default = "default_pet_scale")]
    pet_scale: f32,
    /// 动作间隔档位下标（PET_ACTION_RANGES，区间内随机而非固定周期）
    #[serde(default = "default_pet_action_index")]
    pet_action_index: u8,
    /// 鼠标移出后自动缩回便签/宠物态的宽限毫秒；0 = 不自动缩回
    #[serde(default = "default_pet_collapse_ms")]
    pet_auto_collapse_ms: u32,
    /// 他人全屏时桌宠自动避让
    #[serde(default = "default_true")]
    pet_hide_on_fullscreen: bool,
    /// 桌宠态几何：窗口左上角与尺寸（物理像素）
    #[serde(default)]
    pet_x: Option<i32>,
    #[serde(default)]
    pet_y: Option<i32>,
    /// 便签态几何
    #[serde(default)]
    note_x: Option<i32>,
    #[serde(default)]
    note_y: Option<i32>,
    #[serde(default)]
    note_w: Option<u32>,
    #[serde(default)]
    note_h: Option<u32>,
}

/// 动作间隔档位（秒）：只存档位下标，实际等待时长在区间内随机
const PET_ACTION_RANGES: [(u32, u32); 6] = [
    (15, 30),
    (30, 60),
    (45, 90),
    (60, 120),
    (120, 180),
    (180, 300),
];

/// 桌宠缩放区间。上限收在 1.5：宠物窗做不到逐像素点击穿透（Tauri 只支持整窗
/// setIgnoreCursorEvents），Scale 直接等于压在桌面上的死区尺寸——1.5 约 257×324，
/// 2.0 约 342×432。待实测确认死区可接受后再考虑放宽。
const PET_SCALE_MIN: f32 = 0.5;
const PET_SCALE_MAX: f32 = 1.5;

/// 自动缩回宽限的上限，防止被手改成离谱值后看起来像「缩不回」
const PET_COLLAPSE_MS_MAX: u32 = 5_000;

fn default_true() -> bool {
    true
}

fn default_pet_scale() -> f32 {
    1.0
}

fn default_pet_action_index() -> u8 {
    2
}

fn default_pet_collapse_ms() -> u32 {
    500
}

/// 合法的窗口形态
const APP_FORMS: [&str; 2] = ["pet", "note"];

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            opacity: 0.92,
            yuan_shi: 0,
            mora: 0,
            streak: 0,
            weekly_settled_week: String::new(),
            alarm_interval: 600,
            alarm_duration: 15,
            skin_enabled: false,
            theme: String::from("light"),
            pet_enabled: false,
            form: String::from("note"),
            pet_scale: default_pet_scale(),
            pet_action_index: default_pet_action_index(),
            pet_auto_collapse_ms: default_pet_collapse_ms(),
            pet_hide_on_fullscreen: true,
            pet_x: None,
            pet_y: None,
            note_x: None,
            note_y: None,
            note_w: None,
            note_h: None,
        }
    }
}

fn config_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config.json")
}

fn load_config(app: &tauri::AppHandle) -> AppConfig {
    let path = config_path(app);
    // 主文件损坏（写一半被断电/杀软截断）时退回上一代 .bak，再退默认值，
    // 避免一次读取失败就把用户余额与开关清空。
    let mut config = read_config_file(&path)
        .or_else(|| read_config_file(&config_bak_path(&path)))
        .unwrap_or_default();
    normalize_alarm_config(&mut config);
    normalize_theme(&mut config);
    normalize_form(&mut config);
    normalize_pet_config(&mut config);
    config
}

fn read_config_file(path: &PathBuf) -> Option<AppConfig> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn config_bak_path(path: &PathBuf) -> PathBuf {
    path.with_file_name("config.json.bak")
}

/// 规范化窗口形态：非法或缺失时回退便签态（桌宠化之前只有便签态）
fn normalize_form(config: &mut AppConfig) {
    if !APP_FORMS.contains(&config.form.as_str()) {
        config.form = String::from("note");
    }
}

/// 规范化桌宠参数：越界与 NaN 一律纠偏回默认，而不是让前端拿到 NaN 后崩几何。
fn normalize_pet_config(config: &mut AppConfig) {
    if !config.pet_scale.is_finite()
        || config.pet_scale < PET_SCALE_MIN
        || config.pet_scale > PET_SCALE_MAX
    {
        config.pet_scale = default_pet_scale();
    }
    if config.pet_action_index as usize >= PET_ACTION_RANGES.len() {
        config.pet_action_index = default_pet_action_index();
    }
    if config.pet_auto_collapse_ms > PET_COLLAPSE_MS_MAX {
        config.pet_auto_collapse_ms = default_pet_collapse_ms();
    }
}

/// 规范化闹钟参数：旧版本配置文件缺少字段时 serde 会填 0（字段级默认值），
/// 0 间隔会导致前端定时器空转，这里统一回退到默认值。
fn normalize_alarm_config(config: &mut AppConfig) {
    if config.alarm_interval == 0 {
        config.alarm_interval = AppConfig::default().alarm_interval;
    }
    if config.alarm_duration == 0 {
        config.alarm_duration = AppConfig::default().alarm_duration;
    }
}

/// 合法的界面颜色主题
const APP_THEMES: [&str; 4] = ["light", "sky", "sakura", "green"];

fn is_valid_theme(theme: &str) -> bool {
    APP_THEMES.contains(&theme)
}

/// 规范化颜色主题：非法或缺失（空串）时回退到默认 light
fn normalize_theme(config: &mut AppConfig) {
    if !is_valid_theme(&config.theme) {
        config.theme = AppConfig::default().theme;
    }
}

fn save_config(app: &tauri::AppHandle, config: &AppConfig) {
    let path = config_path(app);
    let data = serde_json::to_string_pretty(config).unwrap_or_default();
    write_config_atomic(&path, &data).ok();
}

/// 原子写配置：先落 .tmp 并 sync，再把旧文件挪成 .bak、.tmp 转正。
/// 原来的裸 fs::write 若在写入途中崩溃/断电会留下截断文件，而桌宠化后 config.json
/// 还要多存几何与开关，损坏面变大；这样任何一步失败都尽量保住原文件可读。
fn write_config_atomic(path: &PathBuf, data: &str) -> std::io::Result<()> {
    let tmp = path.with_file_name("config.json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(data.as_bytes())?;
        file.sync_all()?;
    }
    if path.exists() {
        // Windows 上 rename 覆盖已存在目标会成功（MoveFileEx 带 REPLACE_EXISTING）
        let _ = fs::rename(path, config_bak_path(path));
    }
    fs::rename(&tmp, path)
}

/// 前端启动时一次读齐的形态与几何（省掉一串 get_* 往返）
#[derive(Serialize)]
struct UiState {
    pet_enabled: bool,
    form: String,
    pet_scale: f32,
    /// 动作间隔档位下标：设置面板要能高亮当前档，只回上下限反推不出是第几档
    action_index: u8,
    action_min_sec: u32,
    action_max_sec: u32,
    auto_collapse_ms: u32,
    hide_on_fullscreen: bool,
    pet_x: Option<i32>,
    pet_y: Option<i32>,
    note_x: Option<i32>,
    note_y: Option<i32>,
    note_w: Option<u32>,
    note_h: Option<u32>,
}

/// 记录「当前处于哪个形态 + 该形态的窗口几何」。
/// 宠物态只存位置：尺寸由 pet_scale 与精灵可见包围盒推导，存下来只会在改缩放后失真。
fn apply_ui_state(config: &mut AppConfig, form: &str, x: i32, y: i32, w: Option<u32>, h: Option<u32>) {
    if APP_FORMS.contains(&form) {
        config.form = form.to_string();
    }
    if config.form == "pet" {
        config.pet_x = Some(x);
        config.pet_y = Some(y);
    } else {
        config.note_x = Some(x);
        config.note_y = Some(y);
        config.note_w = w;
        config.note_h = h;
    }
}

/// 关掉桌宠就不可能还停在宠物态：否则下次启动是一个没人管的畸形小窗，
/// 而且托盘/设置面板的开关状态和画面会对不上。托盘切换与 set_pet_config 共用。
fn enforce_pet_form(config: &mut AppConfig) {
    if !config.pet_enabled && config.form == "pet" {
        config.form = String::from("note");
    }
}

/// UiState 的唯一构造点：normalize 之后下标必定在区间内
fn ui_state_of(c: &AppConfig) -> UiState {
    let (min, max) = PET_ACTION_RANGES[c.pet_action_index as usize];
    UiState {
        pet_enabled: c.pet_enabled,
        form: c.form.clone(),
        pet_scale: c.pet_scale,
        action_index: c.pet_action_index,
        action_min_sec: min,
        action_max_sec: max,
        auto_collapse_ms: c.pet_auto_collapse_ms,
        hide_on_fullscreen: c.pet_hide_on_fullscreen,
        pet_x: c.pet_x,
        pet_y: c.pet_y,
        note_x: c.note_x,
        note_y: c.note_y,
        note_w: c.note_w,
        note_h: c.note_h,
    }
}

#[tauri::command]
fn get_ui_state(app: tauri::AppHandle) -> UiState {
    ui_state_of(&load_config(&app))
}

#[tauri::command]
fn save_ui_state(
    app: tauri::AppHandle,
    form: String,
    x: i32,
    y: i32,
    w: Option<u32>,
    h: Option<u32>,
) {
    let mut c = load_config(&app);
    apply_ui_state(&mut c, &form, x, y, w, h);
    save_config(&app, &c);
}

/// 桌宠动作/气泡需要的当天事实。判定完全复用结算那套解析（模块名按标题文本精确
/// 匹配，权威只有一份），这里只读不写。
#[derive(Debug, Serialize, Clone)]
struct TodayProgress {
    modules_done: u32,
    triple: bool,
    /// 三个结算模块里是否至少有一条非空委托（全是空白占位不该被夸）
    has_tasks: bool,
    /// 明日委托（引用块 `>`）是否已填
    tomorrow_filled: bool,
    note_found: bool,
}

fn today_progress_internal(app: &tauri::AppHandle) -> TodayProgress {
    let today = today_ymd();
    let note = read_index(app)
        .into_iter()
        .find(|n| n.created_at.starts_with(&today));
    let Some(note) = note else {
        return TodayProgress {
            modules_done: 0,
            triple: false,
            has_tasks: false,
            tomorrow_filled: false,
            note_found: false,
        };
    };
    let content = load_note_internal(app, &note.id).unwrap_or_default();
    progress_of(&content)
}

/// 从一份委托正文算出当天事实（与 settle_reward 用同一套 section_complete 判定）
fn progress_of(content: &str) -> TodayProgress {
    let sections = parse_md_sections(content);
    let done = SETTLE_MODULES
        .iter()
        .filter(|m| sections.get(**m).map(|s| section_complete(s)).unwrap_or(false))
        .count() as u32;
    let has_tasks = SETTLE_MODULES.iter().filter_map(|m| sections.get(*m)).any(|s| {
        s.lines()
            .map(str::trim_start)
            .any(|l| (l.starts_with("- [") || l.starts_with("* [")) && !is_blank_item(l))
    });
    let tomorrow_filled = content.lines().any(|l| l.trim_start().starts_with('>'));
    TodayProgress {
        modules_done: done,
        triple: done == SETTLE_MODULES.len() as u32,
        has_tasks,
        tomorrow_filled,
        note_found: true,
    }
}

#[tauri::command]
fn get_today_progress(app: tauri::AppHandle) -> TodayProgress {
    today_progress_internal(&app)
}

/// 桌宠设置：一次写齐所有可编辑项，返回规范化后的 UiState。
/// 不做成五个独立命令，是为了让「保存 → normalize → 广播 ui-state-changed → 前端
/// 按权威值重落几何」只有一条通路（前端的 fetchUiState 已经承担这条链）。
#[tauri::command]
fn set_pet_config(
    app: tauri::AppHandle,
    pet_enabled: bool,
    scale: f32,
    action_index: u8,
    auto_collapse_ms: u32,
    hide_on_fullscreen: bool,
) -> UiState {
    use tauri::Emitter;
    let mut c = load_config(&app);
    c.pet_enabled = pet_enabled;
    c.pet_scale = scale;
    c.pet_action_index = action_index;
    c.pet_auto_collapse_ms = auto_collapse_ms;
    c.pet_hide_on_fullscreen = hide_on_fullscreen;
    enforce_pet_form(&mut c);
    normalize_pet_config(&mut c);
    normalize_form(&mut c);
    save_config(&app, &c);
    refresh_tray_menu(&app);
    let _ = app.emit("ui-state-changed", ());
    ui_state_of(&c)
}

/// 用户多久没动键鼠（秒）。桌宠据此决定「现在说话会不会吵到主人」。
#[tauri::command]
fn get_idle_seconds() -> u32 {
    idle_seconds_internal()
}

/// 前台窗口是否占满了它所在显示器（游戏 / 投屏 / 全屏播放）。
/// 只读几何与类名，不枚举进程——桌宠避让不需要知道那是谁。
#[tauri::command]
fn is_foreground_fullscreen(app: tauri::AppHandle) -> bool {
    foreground_fullscreen_internal(&app)
}

/// GetTickCount 是 32 位、约 49.7 天回绕一次的毫秒计数，必须用 wrapping_sub，
/// 否则回绕瞬间会算出一个巨大的负数（转 u32 就是天文数字）→ 桌宠整天不开口。
fn idle_seconds_from_ticks(now: u32, last_input: u32) -> u32 {
    now.wrapping_sub(last_input) / 1000
}

/// 前台窗是否覆盖整块屏幕。允许「更大」（某些全屏实现会超出边界一圈）。
fn rect_covers_monitor(win: [i32; 4], monitor: [i32; 4]) -> bool {
    win[0] <= monitor[0] && win[1] <= monitor[1] && win[2] >= monitor[2] && win[3] >= monitor[3]
}

/// 这些「全屏」窗口不是用户在用某个应用，避让它们没有意义：桌面本体、任务栏、
/// 通知区域溢出窗。把它们算成全屏会让桌宠永远躲着不出来。
const IGNORED_FOREGROUND_CLASSES: [&str; 4] = [
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "NotifyIconOverflowWindow",
];

fn is_ignored_foreground_class(class: &str) -> bool {
    IGNORED_FOREGROUND_CLASSES.contains(&class)
}

#[cfg(windows)]
fn idle_seconds_internal() -> u32 {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // 失败就当「刚动过」，让桌宠闭嘴而不是乱说话
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return 0;
    }
    idle_seconds_from_ticks(unsafe { GetTickCount() }, info.dwTime)
}

#[cfg(not(windows))]
fn idle_seconds_internal() -> u32 {
    0
}

#[cfg(windows)]
fn foreground_fullscreen_internal(app: &tauri::AppHandle) -> bool {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect,
    };

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return false;
    }
    // 自己（含气泡窗）永远不算「他人全屏」，否则桌宠会把自己藏起来
    if let Some(w) = app.get_webview_window("main") {
        if let Ok(own) = w.hwnd() {
            if own == hwnd {
                return false;
            }
        }
    }

    let mut buffer = [0u16; 64];
    let written = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if written <= 0 {
        return false;
    }
    let class = String::from_utf16_lossy(&buffer[..written as usize]);
    if is_ignored_foreground_class(&class) {
        return false;
    }

    let mut win_rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut win_rect) }.is_err() {
        return false;
    }
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY) };
    if monitor.is_invalid() {
        return false;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return false;
    }
    rect_covers_monitor(
        [win_rect.left, win_rect.top, win_rect.right, win_rect.bottom],
        [info.rcMonitor.left, info.rcMonitor.top, info.rcMonitor.right, info.rcMonitor.bottom],
    )
}

#[cfg(not(windows))]
fn foreground_fullscreen_internal(_app: &tauri::AppHandle) -> bool {
    false
}

#[tauri::command]
fn get_yuan_shi(app: tauri::AppHandle) -> u32 {
    load_config(&app).yuan_shi
}

#[tauri::command]
fn get_mora(app: tauri::AppHandle) -> u32 {
    load_config(&app).mora
}

#[tauri::command]
fn get_streak(app: tauri::AppHandle) -> u32 {
    load_config(&app).streak
}

#[tauri::command]
fn get_close_behavior(app: tauri::AppHandle) -> bool {
    load_config(&app).close_to_tray
}

#[tauri::command]
fn set_close_behavior(app: tauri::AppHandle, close_to_tray: bool) {
    let mut config = load_config(&app);
    config.close_to_tray = close_to_tray;
    save_config(&app, &config);
}

#[tauri::command]
fn get_opacity(app: tauri::AppHandle) -> f32 {
    load_config(&app).opacity
}

#[tauri::command]
fn set_opacity(app: tauri::AppHandle, opacity: f32) {
    let mut config = load_config(&app);
    config.opacity = opacity;
    save_config(&app, &config);
}

#[tauri::command]
fn get_theme(app: tauri::AppHandle) -> String {
    load_config(&app).theme
}

#[tauri::command]
fn set_theme(app: tauri::AppHandle, theme: String) {
    if !is_valid_theme(&theme) {
        return;
    }
    let mut config = load_config(&app);
    config.theme = theme;
    save_config(&app, &config);
}

#[tauri::command]
fn get_alarm_interval(app: tauri::AppHandle) -> u64 {
    load_config(&app).alarm_interval
}

#[tauri::command]
fn set_alarm_interval(app: tauri::AppHandle, seconds: u64) {
    let mut config = load_config(&app);
    config.alarm_interval = seconds.max(1);
    save_config(&app, &config);
}

#[tauri::command]
fn get_alarm_duration(app: tauri::AppHandle) -> u64 {
    load_config(&app).alarm_duration
}

#[tauri::command]
fn set_alarm_duration(app: tauri::AppHandle, seconds: u64) {
    let mut config = load_config(&app);
    config.alarm_duration = seconds.clamp(1, 60);
    save_config(&app, &config);
}

// ── Skin (整窗背景图皮肤，仅浅色模式生效) ──

/// 皮肤最大字节数：8 MB
const SKIN_MAX_BYTES: usize = 8 * 1024 * 1024;

/// 皮肤存储目录：app_data_dir()/skins/，活动皮肤固定存为 `active`（无扩展名）
fn skins_dir(app: &tauri::AppHandle) -> PathBuf {
    let path = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("skins");
    fs::create_dir_all(&path).ok();
    path
}

fn skin_path(app: &tauri::AppHandle) -> PathBuf {
    skins_dir(app).join("active")
}

/// 依据文件头魔数识别图片格式，返回对应 mime；无法识别返回 None。
/// 支持 PNG / JPEG / WebP。
fn detect_skin_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.len() >= 12
        && &bytes[0..4] == b"RIFF"
        && &bytes[8..12] == b"WEBP"
    {
        return Some("image/webp");
    }
    None
}

fn encode_skin_data_url(bytes: &[u8], mime: &str) -> String {
    format!("data:{};base64,{}", mime, BASE64.encode(bytes))
}

#[tauri::command]
fn get_skin(app: tauri::AppHandle) -> Result<Option<String>, String> {
    if !load_config(&app).skin_enabled {
        return Ok(None);
    }
    let path = skin_path(&app);
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let mime = match detect_skin_mime(&bytes) {
        Some(m) => m,
        None => return Ok(None),
    };
    Ok(Some(encode_skin_data_url(&bytes, mime)))
}

#[tauri::command]
fn save_skin(app: tauri::AppHandle, data_url: String) -> Result<String, String> {
    let payload = data_url
        .split_once(',')
        .map(|(_, b64)| b64.to_string())
        .unwrap_or(data_url);
    let bytes = BASE64.decode(payload.as_bytes()).map_err(|_| "图片数据解析失败".to_string())?;
    if bytes.len() > SKIN_MAX_BYTES {
        return Err("图片过大（需 ≤ 8MB）".to_string());
    }
    let mime = detect_skin_mime(&bytes)
        .ok_or_else(|| "不支持的图片格式（仅支持 PNG / JPG / WebP）".to_string())?;

    fs::write(skin_path(&app), &bytes).map_err(|e| e.to_string())?;
    let mut config = load_config(&app);
    config.skin_enabled = true;
    save_config(&app, &config);
    Ok(encode_skin_data_url(&bytes, mime))
}

#[tauri::command]
fn remove_skin(app: tauri::AppHandle) {
    let _ = fs::remove_file(skin_path(&app));
    let mut config = load_config(&app);
    config.skin_enabled = false;
    save_config(&app, &config);
}

// ── Update check ──

const GITEE_OWNER: &str = "Honest_Leon";
const GITEE_REPO: &str = "lightweight-sticky-notes";

#[derive(Debug, Serialize, Clone)]
struct UpdateInfo {
    current_version: String,
    latest_version: String,
    has_update: bool,
    /// 安装版安装包（文件名含 setup 的 .exe 附件）
    download_url: String,
    /// 免安装版（绿色版）exe（文件名含 portable 的 .exe 附件）
    portable_download_url: String,
}

/// 从 Release 的 assets 中选取安装包下载地址：
/// setup 附件（文件名含 "setup"）与 portable 附件（文件名含 "portable"，
/// 回退到不含 "setup" 的 .exe）。返回 (setup_url, portable_url)。
fn pick_release_assets(release: &serde_json::Value) -> (Option<String>, Option<String>) {
    let assets = release
        .get("assets")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    let exe_assets: Vec<(String, String)> = assets
        .iter()
        .filter_map(|a| {
            let name = a.get("name").and_then(|n| n.as_str())?.to_string();
            let url = a
                .get("browser_download_url")
                .and_then(|u| u.as_str())?
                .to_string();
            Some((name, url))
        })
        .filter(|(name, _)| name.to_lowercase().ends_with(".exe"))
        .collect();
    let find = |pred: &dyn Fn(&str) -> bool| {
        exe_assets
            .iter()
            .find(|(name, _)| pred(&name.to_lowercase()))
            .map(|(_, url)| url.clone())
    };
    let setup = find(&|n| n.contains("setup"));
    let portable = find(&|n| n.contains("portable"))
        .or_else(|| find(&|n| !n.contains("setup")));
    (setup, portable)
}

fn version_gt(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.trim()
            .trim_start_matches('v')
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    let n = va.len().max(vb.len());
    for i in 0..n {
        let x = va.get(i).copied().unwrap_or(0);
        let y = vb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// 更新检查的内存缓存：命中且未过期则直接返回，避免每次点击都请求 Gitee、
/// 触发匿名接口限流。
#[derive(Clone)]
struct UpdateCache {
    at: Instant,
    result: Result<UpdateInfo, String>,
}

/// 单例缓存，进程生命周期内共享。
static UPDATE_CACHE: OnceLock<Mutex<Option<UpdateCache>>> = OnceLock::new();

/// 成功结果缓存时长：10 分钟内重复检查直接命中缓存。
const UPDATE_CACHE_TTL_OK: Duration = Duration::from_secs(600);
/// 失败结果缓存时长：仅 1 分钟，避免网络瞬断/404 等被长期误缓存，
/// 同时防止连续失败时高频重试。
const UPDATE_CACHE_TTL_ERR: Duration = Duration::from_secs(60);

fn update_cache() -> &'static Mutex<Option<UpdateCache>> {
    UPDATE_CACHE.get_or_init(|| Mutex::new(None))
}

/// 依据结果类型返回对应 TTL：成功长缓存，失败短冷却。
fn cache_ttl(result_is_ok: bool) -> Duration {
    if result_is_ok {
        UPDATE_CACHE_TTL_OK
    } else {
        UPDATE_CACHE_TTL_ERR
    }
}

/// 缓存是否仍有效（未过期）。纯函数，便于测试。
fn cache_fresh(at: Instant, result_is_ok: bool) -> bool {
    at.elapsed() < cache_ttl(result_is_ok)
}

/// 通过 Gitee Release API 检查更新：tag_name 作为最新版本号，
/// 从附件中选取安装包下载地址。结果带 TTL 缓存以降低撞限流概率。
#[tauri::command]
fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    // 命中有效缓存则直接返回（含失败缓存，实现短冷却）
    if let Some(cached) = update_cache().lock().unwrap().clone() {
        if cache_fresh(cached.at, cached.result.is_ok()) {
            return cached.result;
        }
    }

    let fetched = fetch_update_info(&app);
    *update_cache().lock().unwrap() = Some(UpdateCache {
        at: Instant::now(),
        result: fetched.clone(),
    });
    fetched
}

/// 实际向 Gitee 发起请求并解析版本信息（不缓存）。
fn fetch_update_info(app: &tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let url = format!(
        "https://gitee.com/api/v5/repos/{}/{}/releases/latest",
        GITEE_OWNER, GITEE_REPO
    );

    let resp = match ureq::get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(404, _)) => {
            return Err("检查更新失败：仓库尚未发布任何 Release".to_string());
        }
        Err(e) => return Err(format!("检查更新失败：无法连接 gitee（{}）", e)),
    };
    let body = resp
        .into_string()
        .map_err(|e| format!("读取响应失败：{}", e))?;

    let release: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("解析 Release 信息失败：{}", e))?;

    let latest = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .map(|t| t.trim().trim_start_matches('v').to_string())
        .unwrap_or_else(|| current.clone());

    let has_update = version_gt(&latest, &current);

    // 仅在确有更新时才解析附件；无更新时无需下载地址
    let (setup_url, portable_url) = if has_update {
        let (s, p) = pick_release_assets(&release);
        if s.is_none() && p.is_none() {
            return Err(
                "最新 Release 未附带安装包（.exe），请前往 Gitee 发布页手动下载".to_string(),
            );
        }
        (s.unwrap_or_default(), p.unwrap_or_default())
    } else {
        (String::new(), String::new())
    };

    Ok(UpdateInfo {
        current_version: current,
        latest_version: latest,
        has_update,
        download_url: setup_url,
        portable_download_url: portable_url,
    })
}

/// 安装版的候选安装目录（相对 LOCALAPPDATA）。
/// 前两项为更名后（productName=每日委托模拟器）的目录，后三项为旧版
/// 「轻量便签」时期的目录：保留以保证老版本安装后的自更新检测仍有效。
fn installed_dirs(local_appdata: &std::path::Path) -> Vec<PathBuf> {
    vec![
        local_appdata.join("Programs").join("每日委托模拟器"),
        local_appdata.join("每日委托模拟器"),
        local_appdata.join("轻量便签"),
        local_appdata.join("Programs").join("LightweightStickyNotes"),
        local_appdata.join("LightweightStickyNotes"),
    ]
}

/// 判断当前 exe 是否位于安装版的安装目录内（否则视为免安装版）
fn exe_is_installed_version(exe: &std::path::Path, local_appdata: &std::path::Path) -> bool {
    match exe.parent() {
        Some(dir) => installed_dirs(local_appdata)
            .iter()
            .any(|d| dir == d),
        None => false,
    }
}

/// 下载 url 内容到指定文件
fn download_to(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|e| format!("下载失败：{}", e))?;
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| format!("保存安装包失败：{}", e))?;
    Ok(())
}

/// 免安装版延迟替换脚本（cmd）。
/// 在主进程退出后执行：等待解除文件占用 → 旧 exe 改名 .old → 写入新版 →
/// 成功后启动新版并清理，失败则回滚为原程序。非 Windows 不生成。
#[allow(dead_code)] // 仅 Windows 分支使用，Linux 上保留供测试
fn portable_update_script() -> &'static str {
    r#"@echo off
setlocal
if "%~1"=="" exit /b 1
set "TGT=%~f1"
set "NEW=%~f2"
if not exist "%NEW%" exit /b 1
timeout /t 2 /nobreak >nul
ren "%TGT%" "%~nx1.old" >nul 2>&1
copy /y "%NEW%" "%TGT%" >nul 2>&1
if exist "%TGT%" (
  del "%TGT%.old" >nul 2>&1
  start "" "%TGT%"
) else (
  ren "%TGT%.old" "%~nx1" >nul 2>&1
)
del "%NEW%" >nul 2>&1
endlocal
exit /b 0
"#
}

/// 免安装版自替换：不直接在运行中的旧进程里改名/覆盖（会被文件锁拒绝），
/// 而是把替换动作交给独立 bat 脚本延迟到主进程退出后执行。
fn self_replace_portable(
    app: &tauri::AppHandle,
    url: &str,
    exe: &std::path::Path,
) -> Result<(), String> {
    let tmp = std::env::temp_dir().join("lwsn-update");
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let new_exe = tmp.join("portable.exe");
    download_to(url, &new_exe)?;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;

        let bat = tmp.join("update-portable.bat");
        std::fs::write(&bat, portable_update_script())
            .map_err(|e| format!("写入更新脚本失败：{}", e))?;

        std::process::Command::new("cmd")
            .args([
                "/c",
                &bat.to_string_lossy(),
                &exe.to_string_lossy(),
                &new_exe.to_string_lossy(),
            ])
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()
            .map_err(|e| format!("启动更新脚本失败：{}", e))?;

        // 主进程立即让位，脚本在无占用时完成替换并拉起新版本
        app.exit(0);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (app, exe);
        Err("免安装版自更新仅支持 Windows".to_string())
    }
}

/// 安装版：下载 setup 静默安装，安装完成后重启新版本
fn install_via_setup(app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    let tmp = std::env::temp_dir().join("lwsn-update");
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let installer = tmp.join("setup.exe");
    download_to(url, &installer)?;

    let status = std::process::Command::new(&installer)
        .arg("/S")
        .status()
        .map_err(|e| format!("启动安装器失败：{}", e))?;
    if !status.success() {
        return Err("安装未成功，请前往 gitee 手动下载安装".to_string());
    }

    let exe_name = "lightweight-sticky-notes.exe";
    let candidates: Vec<PathBuf> = std::env::var("LOCALAPPDATA")
        .map(|base| installed_dirs(&PathBuf::from(base)))
        .unwrap_or_default()
        .into_iter()
        .map(|d| d.join(exe_name))
        .collect();

    if let Some(new_exe) = candidates.iter().find(|p| p.exists()) {
        std::process::Command::new(new_exe).spawn().ok();
        std::thread::sleep(std::time::Duration::from_millis(800));
        app.exit(0);
    }

    Ok(())
}

#[tauri::command]
fn download_and_install(
    app: tauri::AppHandle,
    setup_url: String,
    portable_url: String,
) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let is_installed = std::env::var("LOCALAPPDATA")
        .map(|base| exe_is_installed_version(&exe, &PathBuf::from(base)))
        .unwrap_or(false);

    if is_installed {
        if setup_url.is_empty() {
            return Err("最新 Release 未附带安装版安装包，请前往 Gitee 发布页手动下载"
                .to_string());
        }
        install_via_setup(&app, &setup_url)
    } else {
        if portable_url.is_empty() {
            return Err("最新 Release 未附带免安装版程序，请前往 Gitee 发布页手动下载"
                .to_string());
        }
        self_replace_portable(&app, &portable_url, &exe)
    }
}

// ── Autostart via registry (reliable, no VBS) ──

fn autostart_reg_key() -> &'static str {
    r"Software\Microsoft\Windows\CurrentVersion\Run"
}

fn autostart_reg_value() -> &'static str {
    "LightweightStickyNotes"
}

fn autostart_reg_value_data(exe_path: &str) -> String {
    format!("\"{}\" --startup", exe_path)
}

fn is_autostart_enabled_internal() -> bool {
    #[cfg(windows)]
    {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey(autostart_reg_key()) {
            key.get_value::<String, _>(autostart_reg_value()).is_ok()
        } else {
            false
        }
    }
    #[cfg(not(windows))]
    false
}

fn enable_autostart_in_registry(exe_path: &str) -> Result<bool, String> {
    #[cfg(debug_assertions)]
    {
        let _ = exe_path;
        Err("开发模式不支持开机自启，请使用发布版本开启".to_string())
    }

    #[cfg(not(debug_assertions))]
    {
        #[cfg(windows)]
        {
            use winreg::enums::*;
            let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
            let key = hkcu
                .open_subkey_with_flags(autostart_reg_key(), KEY_SET_VALUE)
                .map_err(|e| e.to_string())?;
            key.set_value(autostart_reg_value(), &autostart_reg_value_data(exe_path))
                .map_err(|e| e.to_string())?;
            Ok(true)
        }

        #[cfg(not(windows))]
        {
            let _ = exe_path;
            Ok(false)
        }
    }
}

fn disable_autostart_in_registry() {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey_with_flags(autostart_reg_key(), KEY_SET_VALUE) {
            let _ = key.delete_value(autostart_reg_value());
        }
    }
}

fn toggle_autostart_internal() -> Result<bool, String> {
    if is_autostart_enabled_internal() {
        disable_autostart_in_registry();
        Ok(false)
    } else {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        enable_autostart_in_registry(&exe.to_string_lossy())
    }
}

fn migrate_autostart_entry() {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) =
            hkcu.open_subkey_with_flags(autostart_reg_key(), KEY_QUERY_VALUE | KEY_SET_VALUE)
        {
            if let Ok(old) = key.get_value::<String, _>("lightweight-sticky-notes") {
                let exe_path = old.trim().trim_matches('"').to_string();
                if !exe_path.is_empty() && std::path::Path::new(&exe_path).exists() {
                    let _ = key.set_value(
                        autostart_reg_value(),
                        &autostart_reg_value_data(&exe_path),
                    );
                }
                let _ = key.delete_value("lightweight-sticky-notes");
            }
        }
    }
}

fn cleanup_old_autostart(_app: &tauri::AppHandle) {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let vbs_path = PathBuf::from(appdata)
            .join(r"Microsoft\Windows\Start Menu\Programs\Startup")
            .join("lightweight-sticky-notes.vbs");
        if vbs_path.exists() {
            let _ = fs::remove_file(&vbs_path);
        }
    }

    #[cfg(windows)]
    {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey_with_flags(autostart_reg_key(), winreg::enums::KEY_SET_VALUE) {
            let _ = key.delete_value("load-autostart");
            let _ = key.delete_value("lightweight-sticky-notes");
        }
    }
}

#[tauri::command]
fn cleanup_old_autostart_cmd(app: tauri::AppHandle) {
    cleanup_old_autostart(&app);
}

#[tauri::command]
fn enable_autostart_cmd() -> Result<bool, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    enable_autostart_in_registry(&exe.to_string_lossy())
}

#[tauri::command]
fn disable_autostart_cmd() -> Result<bool, String> {
    disable_autostart_in_registry();
    Ok(false)
}

#[tauri::command]
fn is_autostart_enabled_cmd() -> bool {
    is_autostart_enabled_internal()
}

/// 托盘里「桌宠模式」的菜单文案
fn pet_menu_label(app: &tauri::AppHandle) -> String {
    if load_config(app).pet_enabled {
        String::from("桌宠模式：开")
    } else {
        String::from("桌宠模式：关")
    }
}

fn autostart_menu_label() -> String {
    if is_autostart_enabled_internal() {
        String::from("开机自启：开")
    } else {
        String::from("开机自启：关")
    }
}

/// 托盘的唯一 id：改文案要靠它找回 TrayIcon
const TRAY_ID: &str = "main";

/// 按当前配置重建托盘菜单。菜单文案是配置的投影，凡是改了配置的地方都重建一次，
/// 而不是抓着旧的 MenuItem 句柄 set_text——设置面板那条路径拿不到句柄，而且换过菜单
/// 之后旧句柄指向的项已经不在屏上了，改了也看不见。
fn tray_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};

    let show = MenuItemBuilder::with_id("show", "显示窗口").build(app)?;
    let pet = MenuItemBuilder::with_id("pet", pet_menu_label(app)).build(app)?;
    let autostart = MenuItemBuilder::with_id("autostart", autostart_menu_label()).build(app)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;

    MenuBuilder::new(app)
        .items(&[&show, &pet, &autostart, &separator, &quit])
        .build()
}

fn refresh_tray_menu(app: &tauri::AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = tray_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// 显示主窗（可选抢焦点），并告诉前端「画面又可见了」。
/// 桌宠的动画必须跟着可见性暂停/恢复：隐藏到托盘后还继续解码就是白烧 CPU。
/// hide/show 的调用点分散在托盘与窗口事件里，所以统一从这个函数走。
fn reveal_main(app: &tauri::AppHandle, focus: bool) {
    use tauri::Emitter;

    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    // 最小化的窗光靠 show() 叫不回来：SW_SHOW 只解除「隐藏」，不解除「最小化」，
    // 所以点标题栏最小化之后再点托盘「显示窗口」会毫无反应。必须先 unminimize。
    let _ = w.unminimize();
    let _ = w.show();
    if focus {
        let _ = w.set_focus();
    }
    let _ = app.emit("pet-visibility-changed", true);
}

/// 隐藏主窗到托盘，并通知前端暂停动画
fn hide_main_to_tray(app: &tauri::AppHandle) {
    use tauri::Emitter;

    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
        let _ = app.emit("pet-visibility-changed", false);
    }
}

fn build_tray(app: &tauri::App) -> Result<tauri::tray::TrayIcon, Box<dyn std::error::Error>> {
    use tauri::Emitter;
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let icon = app.default_window_icon().unwrap().clone();
    let menu = tray_menu(&app.handle().clone())?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .tooltip("每日委托模拟器")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "show" => reveal_main(app, true),
                "pet" => {
                    // 只改配置并通知前端，形态与几何一律由前端的 setForm 落位——
                    // 窗口尺寸/焦点这套调用必须只有一个主人，否则会和 CSS 动画抢时序
                    let mut config = load_config(app);
                    config.pet_enabled = !config.pet_enabled;
                    enforce_pet_form(&mut config);
                    save_config(app, &config);
                    refresh_tray_menu(app);
                    // 刚打开桌宠时窗口可能正藏在托盘里，不显示出来用户看不到任何变化
                    reveal_main(app, false);
                    let _ = app.emit("ui-state-changed", ());
                }
                "autostart" => {
                    toggle_autostart_internal().ok();
                    refresh_tray_menu(app);
                }
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                reveal_main(tray.app_handle(), true);
            }
        })
        .build(app)?;

    Ok(tray)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_title_uses_created_date() {
        assert_eq!(format_daily_title("2026-08-16T10:00:00"), "0816");
        assert_eq!(format_daily_title("2026-01-05T10:00:00"), "0105");
    }

    #[test]
    fn section_complete_rules() {
        // All checked -> complete
        assert!(section_complete("- [x] a\n- [x] b\n"));
        // Mixed -> not complete
        assert!(!section_complete("- [x] a\n- [ ] b\n"));
        // Empty -> NOT complete (empty doesn't count as fulfilled)
        assert!(!section_complete(""));
        // Non-checkbox text only -> not complete (no real task)
        assert!(!section_complete("just some notes\n"));
        // Asterisk markers also count
        assert!(section_complete("* [x] a\n* [x] b\n"));
    }

    #[test]
    fn section_complete_ignores_blank_items() {
        // 仅空白项 -> 无实项 -> 不完成
        assert!(!section_complete("- [ ] \n"));
        assert!(!section_complete("- [ ]\n"));
        // 已勾选的空白项同样忽略
        assert!(!section_complete("- [x] \n"));
        // 实项全勾 + 空白项 -> 完成
        assert!(section_complete("- [x] A\n- [ ] \n"));
        // 实项未勾 + 空白项 -> 不完成
        assert!(!section_complete("- [ ] A\n- [ ] \n"));
        // 实项有勾有未勾 -> 不完成
        assert!(!section_complete("- [x] A\n- [ ] B\n"));
    }

    #[test]
    fn ensure_task_trailing_space_rules() {
        // 裸复选框补尾空格
        assert_eq!(ensure_task_trailing_space("- [ ]"), "- [ ] ");
        assert_eq!(ensure_task_trailing_space("- [x]"), "- [x] ");
        assert_eq!(ensure_task_trailing_space("* [x]"), "* [x] ");
        // 已带空格 / 有内容 / 普通行不变
        assert_eq!(ensure_task_trailing_space("- [ ] "), "- [ ] ");
        assert_eq!(ensure_task_trailing_space("- [ ] A"), "- [ ] A");
        assert_eq!(ensure_task_trailing_space("普通行"), "普通行");
        // 多行混合且保留末尾换行
        assert_eq!(
            ensure_task_trailing_space("#### 必做\n- [ ]\n- [x] A\n"),
            "#### 必做\n- [ ] \n- [x] A\n"
        );
    }

    #[test]
    fn inherit_skips_blank_items() {
        // 前一天各栏含 2 个空白项 + 1 个实项（旧模板形态）
        let prev = "### 每日委托\n\n#### 必做\n\n- [ ] \n- [ ] \n- [x] A\n\n#### 学习/工作\n\n- [ ] \n- [ ] \n- [ ] B\n\n#### 生活/其他\n\n- [ ] \n- [ ] \n\n#### 随手小记\n\n- \n- \n";
        let result = inherit_from_prev(prev);
        // 实项被继承（必做重置勾选）
        assert!(result.contains("- [ ] A"));
        assert!(result.contains("- [ ] B"));
        // 空白项不重复带入：统计结果中 "- [ ] "（空白行）次数
        let blank_count = result
            .lines()
            .filter(|l| is_blank_item(l))
            .count();
        // 模板兜底只在生活/其他（无有效继承）出现 1 个空白项；必做/学习有实项则无空白项
        assert_eq!(blank_count, 1);
    }

    #[test]
    fn normalize_alarm_config_recovers_zero_values() {
        // 旧配置缺失字段时 serde 填 0 → 回退默认
        let mut c = AppConfig::default();
        c.alarm_interval = 0;
        c.alarm_duration = 0;
        normalize_alarm_config(&mut c);
        assert_eq!(c.alarm_interval, 600);
        assert_eq!(c.alarm_duration, 15);
        // 正常值不受影响
        let mut ok = AppConfig::default();
        ok.alarm_interval = 120;
        ok.alarm_duration = 30;
        normalize_alarm_config(&mut ok);
        assert_eq!(ok.alarm_interval, 120);
        assert_eq!(ok.alarm_duration, 30);
    }

    #[test]
    fn normalize_theme_falls_back_on_invalid() {
        // 非法主题（如 "dark"）→ 回退 light
        let mut c = AppConfig::default();
        c.theme = String::from("dark");
        normalize_theme(&mut c);
        assert_eq!(c.theme, "light");
    }

    #[test]
    fn normalize_theme_keeps_valid_value() {
        // 合法主题保持不变
        let mut c = AppConfig::default();
        c.theme = String::from("sky");
        normalize_theme(&mut c);
        assert_eq!(c.theme, "sky");
    }

    #[test]
    fn normalize_theme_falls_back_on_empty() {
        // 空串（旧配置缺失字段时 serde 填空串）→ 回退 light
        let mut c = AppConfig::default();
        c.theme = String::new();
        normalize_theme(&mut c);
        assert_eq!(c.theme, "light");
    }

    #[test]
    fn normalize_pet_config_clamps_out_of_range() {
        let mut c = AppConfig::default();
        // 超上限 / 低于下限 / NaN / inf 一律回默认，绝不让前端拿到 NaN 去算几何
        c.pet_scale = 2.0;
        normalize_pet_config(&mut c);
        assert_eq!(c.pet_scale, 1.0);
        c.pet_scale = 0.1;
        normalize_pet_config(&mut c);
        assert_eq!(c.pet_scale, 1.0);
        c.pet_scale = f32::NAN;
        normalize_pet_config(&mut c);
        assert!(c.pet_scale.is_finite());
        // 档位越界回默认档
        c.pet_action_index = 99;
        normalize_pet_config(&mut c);
        assert_eq!(c.pet_action_index, default_pet_action_index());
        // 宽限期离谱（手改 config.json）回默认
        c.pet_auto_collapse_ms = PET_COLLAPSE_MS_MAX + 1;
        normalize_pet_config(&mut c);
        assert_eq!(c.pet_auto_collapse_ms, default_pet_collapse_ms());
    }

    #[test]
    fn normalize_pet_config_keeps_valid_values() {
        let mut c = AppConfig::default();
        c.pet_scale = 1.4;
        c.pet_action_index = 5;
        c.pet_auto_collapse_ms = 3_000;
        normalize_pet_config(&mut c);
        assert_eq!(c.pet_scale, 1.4);
        assert_eq!(c.pet_action_index, 5);
        assert_eq!(c.pet_auto_collapse_ms, 3_000);
        // 0 是「不自动缩回」的合法取值，不能被当成「缺失」纠偏掉
        c.pet_auto_collapse_ms = 0;
        normalize_pet_config(&mut c);
        assert_eq!(c.pet_auto_collapse_ms, 0);
    }

    #[test]
    fn normalize_form_falls_back_to_note() {
        let mut c = AppConfig::default();
        assert_eq!(c.form, "note");
        c.form = String::from("bogus");
        normalize_form(&mut c);
        assert_eq!(c.form, "note");
        c.form = String::from("pet");
        normalize_form(&mut c);
        assert_eq!(c.form, "pet");
    }

    #[test]
    fn disabling_pet_forces_note_form() {
        let mut c = AppConfig::default();
        c.pet_enabled = true;
        c.form = String::from("pet");
        enforce_pet_form(&mut c);
        assert_eq!(c.form, "pet");

        c.pet_enabled = false;
        enforce_pet_form(&mut c);
        assert_eq!(c.form, "note");
    }

    #[test]
    fn idle_seconds_survives_tick_wraparound() {
        assert_eq!(idle_seconds_from_ticks(1_000, 0), 1);
        assert_eq!(idle_seconds_from_ticks(65_000, 5_000), 60);
        // GetTickCount 约 49.7 天回绕一次。回绕点之后若用减法会得到负数，
        // 转成 u32 就是四十多亿秒——桌宠会从此「主人永远不在空闲期」地闭嘴。
        assert_eq!(idle_seconds_from_ticks(2_000, u32::MAX - 3_000), 5);
    }

    #[test]
    fn fullscreen_requires_covering_the_monitor() {
        let screen = [0, 0, 1920, 1080];
        assert!(rect_covers_monitor([0, 0, 1920, 1080], screen));
        // 略超出边界的全屏实现（部分游戏/投屏）仍然算
        assert!(rect_covers_monitor([-1, -1, 1921, 1081], screen));
        // 多屏副屏上的全屏：窗口坐标相对整块桌面，显示器矩形也相对整块桌面
        assert!(rect_covers_monitor([1920, 0, 3840, 1080], [1920, 0, 3840, 1080]));
        // 让出任务栏（工作区尺寸）不是全屏，不该因此躲起来
        assert!(!rect_covers_monitor([0, 0, 1920, 1040], screen));
        // 普通最大化窗口的边框/标题栏在屏幕外一点点是可能的，但右下角必须盖住
        assert!(!rect_covers_monitor([8, 8, 1912, 1072], screen));
    }

    #[test]
    fn desktop_shell_is_not_somebody_elses_fullscreen() {
        assert!(is_ignored_foreground_class("Progman"));
        assert!(is_ignored_foreground_class("WorkerW"));
        assert!(is_ignored_foreground_class("Shell_TrayWnd"));
        assert!(!is_ignored_foreground_class("Chrome_WidgetWin_1"));
        assert!(!is_ignored_foreground_class(""));
    }

    #[test]
    fn old_config_without_pet_fields_loads_with_defaults() {
        // 桌宠化之前的 config.json 原样（无任何 pet_* 字段）必须能无缝升级：
        // 缺失的桌宠开关取 false、形态取 note，用户不会莫名其妙进入桌宠态
        let legacy = r#"{"close_to_tray":false,"opacity":0.5,"yuan_shi":88,"mora":300,
            "streak":2,"weekly_settled_week":"2026-10-12","alarm_interval":600,
            "alarm_duration":15,"skin_enabled":true,"theme":"sky"}"#;
        let mut c: AppConfig = serde_json::from_str(legacy).unwrap();
        normalize_alarm_config(&mut c);
        normalize_theme(&mut c);
        normalize_form(&mut c);
        normalize_pet_config(&mut c);
        assert_eq!(c.yuan_shi, 88);
        assert_eq!(c.mora, 300);
        assert_eq!(c.theme, "sky");
        assert!(!c.pet_enabled);
        assert_eq!(c.form, "note");
        assert_eq!(c.pet_scale, 1.0);
        assert!(c.pet_hide_on_fullscreen);
        assert!(c.pet_x.is_none());
    }

    #[test]
    fn pet_config_survives_json_roundtrip() {
        let mut c = AppConfig::default();
        c.pet_enabled = true;
        c.form = String::from("pet");
        c.pet_scale = 1.3;
        c.pet_action_index = 4;
        c.pet_auto_collapse_ms = 1_200;
        c.pet_x = Some(-120);
        c.note_w = Some(620);
        let json = serde_json::to_string(&c).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert!(back.pet_enabled);
        assert_eq!(back.form, "pet");
        assert_eq!(back.pet_scale, 1.3);
        assert_eq!(back.pet_action_index, 4);
        assert_eq!(back.pet_auto_collapse_ms, 1_200);
        assert_eq!(back.pet_x, Some(-120));
        assert_eq!(back.note_w, Some(620));
    }

    #[test]
    fn action_index_maps_to_a_real_range() {
        for i in 0..PET_ACTION_RANGES.len() as u8 {
            let mut c = AppConfig::default();
            c.pet_action_index = i;
            normalize_pet_config(&mut c);
            let (min, max) = PET_ACTION_RANGES[c.pet_action_index as usize];
            assert!(min < max, "档位区间应递增，实际 {min}~{max}");
        }
    }

    #[test]
    fn write_config_atomic_keeps_previous_generation() {
        let dir = std::env::temp_dir().join(format!(
            "ltn-config-test-{}-{}",
            std::process::id(),
            file!().len()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        let mut c = AppConfig::default();
        c.yuan_shi = 10;
        let v1 = serde_json::to_string_pretty(&c).unwrap();
        write_config_atomic(&path, &v1).unwrap();
        assert!(path.exists());
        assert!(!config_bak_path(&path).exists());

        c.yuan_shi = 20;
        let v2 = serde_json::to_string_pretty(&c).unwrap();
        write_config_atomic(&path, &v2).unwrap();

        // 正式文件是最新内容，上一代留在 .bak
        let main = fs::read_to_string(&path).unwrap();
        assert!(main.contains("\"yuan_shi\": 20"));
        let bak = fs::read_to_string(config_bak_path(&path)).unwrap();
        assert!(bak.contains("\"yuan_shi\": 10"));
        // 临时文件不应残留
        assert!(!dir.join("config.json.tmp").exists());

        // 主文件被截断时，读取应退回 .bak 而不是清空余额
        fs::write(&path, "{\"close_to_tray\":true,\"opac").unwrap();
        let recovered: AppConfig =
            read_config_file(&path).or_else(|| read_config_file(&config_bak_path(&path))).unwrap();
        assert_eq!(recovered.yuan_shi, 10);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_ui_state_pet_stores_position_only() {
        let mut c = AppConfig::default();
        apply_ui_state(&mut c, "pet", -120, 640, Some(620), Some(500));
        assert_eq!(c.form, "pet");
        assert_eq!(c.pet_x, Some(-120));
        assert_eq!(c.pet_y, Some(640));
        // 宠物态尺寸由 pet_scale 推导，不该被顺手写进便签态那套字段
        assert_eq!(c.note_w, None);
        assert_eq!(c.note_x, None);
    }

    #[test]
    fn apply_ui_state_note_stores_full_geometry() {
        let mut c = AppConfig::default();
        c.form = String::from("pet");
        c.pet_x = Some(10);
        c.pet_y = Some(20);
        apply_ui_state(&mut c, "note", 300, 120, Some(700), Some(560));
        assert_eq!(c.form, "note");
        assert_eq!(
            (c.note_x, c.note_y, c.note_w, c.note_h),
            (Some(300), Some(120), Some(700), Some(560))
        );
        // 宠物态那套保持原值，切换形态不该互相踩
        assert_eq!(c.pet_x, Some(10));
    }

    #[test]
    fn apply_ui_state_ignores_unknown_form() {
        let mut c = AppConfig::default();
        c.form = String::from("note");
        // 非法 form 不改形态，几何写进「当前形态」那一套，不会丢数据
        apply_ui_state(&mut c, "bogus", 1, 2, Some(3), Some(4));
        assert_eq!(c.form, "note");
        assert_eq!(c.note_x, Some(1));
        assert_eq!(c.pet_x, None);
    }

    #[test]
    fn progress_of_matches_settlement_semantics() {
        let full = "### 每日委托\n\n#### 必做\n\n- [x] A\n\n#### 学习/工作\n\n- [x] B\n\n#### 生活/其他\n\n- [x] C\n\n#### 随手小记\n\n- 灵感\n";
        let p = progress_of(full);
        assert_eq!(p.modules_done, 3);
        assert!(p.triple);
        assert!(p.has_tasks);
        // 没写明日委托 -> 该提醒
        assert!(!p.tomorrow_filled);

        let partial = "### 每日委托\n\n#### 必做\n\n- [x] A\n\n#### 学习/工作\n\n- [ ] B\n\n#### 生活/其他\n\n- [ ] \n\n#### 随手小记\n\n- 灵感\n";
        let p = progress_of(partial);
        assert_eq!(p.modules_done, 1);
        assert!(!p.triple);
        assert!(p.has_tasks);

        // 全是空白占位：一条实项都没有，has_tasks=false（宠物不该夸空气）
        let blank = "### 每日委托\n\n#### 必做\n\n- [ ] \n\n#### 学习/工作\n\n- [ ] \n\n#### 生活/其他\n\n- [ ] \n\n#### 随手小记\n\n- \n";
        let p = progress_of(blank);
        assert_eq!(p.modules_done, 0);
        assert!(!p.has_tasks);

        // 明日委托是引用块 >，出现即算已填
        let with_tomorrow = "### 每日委托\n\n#### 必做\n\n- [ ] A\n\n#### 学习/工作\n\n- [ ] B\n> 明天交报告\n\n#### 生活/其他\n\n- [ ] \n";
        let p = progress_of(with_tomorrow);
        assert!(p.tomorrow_filled);
    }

    #[test]
    fn portable_update_script_has_critical_steps() {
        let s = portable_update_script();
        // 关键替换步骤（后缀统一为 .exe.old）
        assert!(s.contains("ren \"%TGT%\" \"%~nx1.old\""));
        assert!(s.contains("copy /y \"%NEW%\" \"%TGT%\""));
        // 成功后启动新版 & 清理旧版（删除路径与改名产物一致）
        assert!(s.contains("start \"\" \"%TGT%\""));
        assert!(s.contains("del \"%TGT%.old\""));
        // 失败回滚为原程序（REN 目标为不含路径的纯文件名）
        assert!(s.contains("ren \"%TGT%.old\" \"%~nx1\""));
        // 等待解除占用 / 隐藏窗口
        assert!(s.contains("timeout /t 2 /nobreak"));
        // 后缀一致性：改名产物、删除、回滚必须都指向 .exe.old（防 xxx.old 残留/回滚失效）
        let old_refs = s.matches("%TGT%.old").count();
        assert!(old_refs >= 2, "删除与回滚均应引用 .exe.old，实际 {old_refs} 处");
        assert!(s.contains("%~nx1.old"), "改名产物应为 <name>.exe.old");
        assert!(!s.contains("%~n1.old"), "不得再使用无扩展名的 <name>.old");
    }

    #[test]
    fn weekly_settle_is_idempotent_across_calls() {
        // 第一次结算（本周未结算）：达标发奖 100*(streak+1)，连续周数 +1
        let (e1, r1, s1) = weekly_settle_logic(5, 2, false);
        assert!(e1);
        assert_eq!(r1, 300);
        assert_eq!(s1, 3);

        // 同周再次查看：不再发奖，streak 保持不变（幂等）
        let (e2, r2, s2) = weekly_settle_logic(5, s1, true);
        assert!(e2);
        assert_eq!(r2, 0);
        assert_eq!(s2, 3);

        // 未达标当周：清零连续周数；之后同周不再重复改动
        let (e3, r3, s3) = weekly_settle_logic(3, 5, false);
        assert!(!e3);
        assert_eq!(r3, 0);
        assert_eq!(s3, 0);
        let (e4, r4, s4) = weekly_settle_logic(3, s3, true);
        assert!(!e4);
        assert_eq!(r4, 0);
        assert_eq!(s4, 0);
    }

    #[test]
    fn settle_reward_values() {
        let sections = |n: u32| -> std::collections::HashMap<String, String> {
            let mut m = std::collections::HashMap::new();
            for (i, name) in SETTLE_MODULES.iter().enumerate() {
                if i < n as usize {
                    m.insert(name.to_string(), "- [x] done\n".to_string());
                } else {
                    m.insert(name.to_string(), "- [ ] todo\n".to_string());
                }
            }
            m
        };
        assert_eq!(settle_reward(&sections(0)).0, 0);
        assert_eq!(settle_reward(&sections(1)).0, 10);
        assert_eq!(settle_reward(&sections(2)).0, 20);
        assert_eq!(settle_reward(&sections(3)).0, 50);
        assert_eq!(settle_reward(&sections(3)).1, true);
    }

    #[test]
    fn mora_reward_values() {
        use std::collections::HashMap;
        let mut m = HashMap::new();
        // 示例：必做 5 条已勾选 + 学习/工作 2 条 + 生活/其他 3 条 → 1000
        m.insert(
            "必做".to_string(),
            "- [x] 1\n- [x] 2\n- [x] 3\n- [x] 4\n- [x] 5\n".to_string(),
        );
        m.insert(
            "学习/工作".to_string(),
            "- [x] a\n- [x] b\n".to_string(),
        );
        m.insert(
            "生活/其他".to_string(),
            "- [x] A\n- [x] B\n- [x] C\n".to_string(),
        );
        assert_eq!(mora_reward(&m), 100 * 10);

        // 未勾选、空白勾选项、引用块不计入；只数已勾选项
        let mut m2 = HashMap::new();
        m2.insert(
            "必做".to_string(),
            "- [x] done\n- [ ] todo\n- [x]\n> 明日委托\n".to_string(),
        );
        m2.insert(
            "学习/工作".to_string(),
            "- [x]\n* [x] star\n".to_string(),
        );
        m2.insert("生活/其他".to_string(), "- [ ] todo2\n".to_string());
        // 计入：done、* [x] star 共 2 项；`- [x]` 空项是空白项、`> 明日委托` 是引用块，均不计
        assert_eq!(mora_reward(&m2), 200);

        // 模块缺失按 0 计
        assert_eq!(mora_reward(&HashMap::new()), 0);
    }

    #[test]
    fn inherit_carries_only_unfinished_and_keeps_free_form_blank() {
        let prev = "### 每日委托\n\n#### 必做\n\n- [x] A\n- [ ] B\n\n#### 学习/工作\n\n- [ ] C\n\n#### 生活/其他\n\n- [x] D\n\n#### 随手小记\n\n- 昨天的灵感\n";
        let result = inherit_from_prev(prev);
        // 随手小记 heading appears exactly once (no duplicate-heading bug).
        assert_eq!(result.matches("#### 随手小记").count(), 1);
        // 必做 carries every item, reset to unchecked.
        assert!(result.contains("- [ ] A"));
        assert!(result.contains("- [ ] B"));
        // 学习/工作 carries only the unchecked item.
        assert!(result.contains("- [ ] C"));
        // 生活/其他 had everything checked → falls back to a blank placeholder.
        assert!(!result.contains("- [x] D"));
        // 随手小记 does not inherit yesterday's free-form text.
        assert!(!result.contains("昨天的灵感"));
    }

    #[test]
    fn inherit_turns_tomorrow_quotes_into_tasks() {
        let prev = "### 每日委托\n\n#### 必做\n\n- [x] A\n\n#### 学习/工作\n\n- [ ] C\n> 明日做X\n\n#### 生活/其他\n\n> 明天买\n\n#### 随手小记\n\n- 灵感\n";
        let result = inherit_from_prev(prev);
        // 明日委托（引用块 >）在新一天转为普通任务 - [ ]
        assert!(result.contains("- [ ] 明日做X"));
        assert!(result.contains("- [ ] 明天买"));
        // 必做全部继承并重置勾选
        assert!(result.contains("- [ ] A"));
        // 学习/工作 未完成项照常继承
        assert!(result.contains("- [ ] C"));
        // 新一天内容不再保留引用块符号
        assert!(!result.contains('>'));
        // 随手小记 不继承
        assert!(!result.contains("灵感"));
    }

    #[test]
    fn registry_value_data_has_quotes_and_startup_flag() {
        let data = autostart_reg_value_data(r"C:\Program Files\轻量便签\app.exe");
        assert_eq!(data, r#""C:\Program Files\轻量便签\app.exe" --startup"#);
    }

    #[test]
    fn value_name_is_not_legacy_name_deleted_by_cleanup() {
        assert_ne!(autostart_reg_value(), "lightweight-sticky-notes");
        assert_eq!(autostart_reg_value(), "LightweightStickyNotes");
    }

    #[test]
    fn migration_parses_legacy_bare_path() {
        let legacy = r"C:\Users\liu\AppData\Local\Programs\LightweightStickyNotes\轻量便签.exe";
        let exe_path = legacy.trim().trim_matches('"').to_string();
        assert_eq!(
            autostart_reg_value_data(&exe_path),
            format!("\"{}\" --startup", legacy)
        );
    }

    #[test]
    fn pick_release_assets_rules() {
        // setup + portable 附件分别选中
        let release: serde_json::Value = serde_json::from_str(
            r#"{"tag_name":"v0.2.0","assets":[
                {"name":"更新说明.md","browser_download_url":"https://gitee.com/x/notes.md"},
                {"name":"轻量便签_0.2.0_x64-setup.exe","browser_download_url":"https://gitee.com/x/setup.exe"},
                {"name":"轻量便签_0.2.0_x64-portable.exe","browser_download_url":"https://gitee.com/x/portable.exe"}
            ]}"#,
        )
        .unwrap();
        let (setup, portable) = pick_release_assets(&release);
        assert_eq!(setup.as_deref(), Some("https://gitee.com/x/setup.exe"));
        assert_eq!(portable.as_deref(), Some("https://gitee.com/x/portable.exe"));

        // 无 portable 附件时回退到不含 setup 的 .exe
        let fallback: serde_json::Value = serde_json::from_str(
            r#"{"assets":[
                {"name":"app-setup.exe","browser_download_url":"https://gitee.com/x/s.exe"},
                {"name":"lightweight-sticky-notes.exe","browser_download_url":"https://gitee.com/x/app.exe"}
            ]}"#,
        )
        .unwrap();
        let (setup, portable) = pick_release_assets(&fallback);
        assert_eq!(setup.as_deref(), Some("https://gitee.com/x/s.exe"));
        assert_eq!(portable.as_deref(), Some("https://gitee.com/x/app.exe"));

        // 无 .exe 附件 / 无 assets 字段
        let no_exe: serde_json::Value =
            serde_json::from_str(r#"{"assets":[{"name":"a.md","browser_download_url":"u"}]}"#)
                .unwrap();
        assert_eq!(pick_release_assets(&no_exe), (None, None));
        let no_assets: serde_json::Value =
            serde_json::from_str(r#"{"tag_name":"v0.2.0"}"#).unwrap();
        assert_eq!(pick_release_assets(&no_assets), (None, None));
    }

    #[test]
    fn exe_is_installed_version_rules() {
        let base = PathBuf::from(r"C:\Users\u\AppData\Local");
        // 位于安装目录 → 安装版
        assert!(exe_is_installed_version(
            &base.join("Programs").join("LightweightStickyNotes").join("app.exe"),
            &base
        ));
        assert!(exe_is_installed_version(&base.join("轻量便签").join("app.exe"), &base));
        // 更名后的新安装目录同样识别为安装版
        assert!(exe_is_installed_version(
            &base.join("Programs").join("每日委托模拟器").join("app.exe"),
            &base
        ));
        assert!(exe_is_installed_version(&base.join("每日委托模拟器").join("app.exe"), &base));
        // 位于任意其他目录（绿色版）→ 免安装版
        assert!(!exe_is_installed_version(
            &PathBuf::from(r"D:\tools\app.exe"),
            &base
        ));
        // 子目录不算安装版（安装版 exe 直接位于安装目录下）
        assert!(!exe_is_installed_version(
            &base.join("Programs").join("LightweightStickyNotes").join("sub").join("app.exe"),
            &base
        ));
    }

    #[test]
    fn version_comparison() {
        assert!(version_gt("0.1.2", "0.1.1"));
        assert!(version_gt("0.2.0", "0.1.9"));
        assert!(version_gt("0.1.10", "0.1.9"));
        assert!(version_gt("1.0.0", "0.9.9"));
        assert!(!version_gt("0.1.1", "0.1.1"));
        assert!(!version_gt("0.1.0", "0.1.1"));
        assert!(!version_gt("0.0.9", "0.1.0"));
    }

    #[test]
    fn update_cache_ttl_differs_by_result_type() {
        // 成功结果长缓存（10 分钟）
        assert_eq!(cache_ttl(true), Duration::from_secs(600));
        // 失败结果短冷却（1 分钟）
        assert_eq!(cache_ttl(false), Duration::from_secs(60));
    }

    #[test]
    fn update_cache_is_fresh_within_ttl() {
        // 刚写入的缓存有效
        let ok_at = Instant::now();
        assert!(cache_fresh(ok_at, true));
        assert!(cache_fresh(ok_at, false));
        // 用绝对远古时间模拟过期
        let very_old = Instant::now() - Duration::from_secs(3600);
        assert!(!cache_fresh(very_old, true));
        assert!(!cache_fresh(very_old, false));
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    // 桌宠化后一个窗口有两种形态，插件不能再记几何：若退出时恰好是
                    // 宠物态，下次启动会恢复成一个被 minWidth 夹住的畸形小窗。
                    // 形态与两套几何由我们自己存（config.json 的 form / *_x / *_y）。
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::VISIBLE
                        & !tauri_plugin_window_state::StateFlags::SIZE
                        & !tauri_plugin_window_state::StateFlags::POSITION,
                )
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_notes,
            delete_note,
            rename_note,
            save_note,
            load_note,
            init_daily,
            check_weekly,
            get_yuan_shi,
            get_mora,
            get_streak,
            get_close_behavior,
            set_close_behavior,
            enable_autostart_cmd,
            disable_autostart_cmd,
            is_autostart_enabled_cmd,
            cleanup_old_autostart_cmd,
            get_opacity,
            set_opacity,
            get_theme,
            set_theme,
            get_alarm_interval,
            set_alarm_interval,
            get_alarm_duration,
            set_alarm_duration,
            get_skin,
            save_skin,
            remove_skin,
            get_ui_state,
            save_ui_state,
            get_today_progress,
            set_pet_config,
            get_idle_seconds,
            is_foreground_fullscreen,
            check_update,
            download_and_install,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            migrate_autostart_entry();
            cleanup_old_autostart(&app_handle);

            // 清理上次免安装版自更新留下的 .exe.old（旧进程可能尚未退出，失败则下次再清）
            if let Ok(exe) = std::env::current_exe() {
                let _ = fs::remove_file(exe.with_extension("exe.old"));
            }

            let _tray = build_tray(app)?;

            let window = app.get_webview_window("main").unwrap();

            if !std::env::args().any(|a| a == "--startup") {
                let _ = window.show();
            }

            let event_handle = app_handle.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let config = load_config(&event_handle);
                    if config.close_to_tray {
                        api.prevent_close();
                        hide_main_to_tray(&event_handle);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
