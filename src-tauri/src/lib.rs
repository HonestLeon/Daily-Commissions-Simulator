use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use chrono::{Local, Datelike};
use tauri::Manager;

// ── Daily commission ──

/// Fixed structure of a daily commission note. The three settled modules are
/// 必做 / 学习·工作 / 生活·其他; 随手小记 is a free-form area, not settled.
const DAILY_CONTENT: &str = "### 每日委托\n\n#### 必做\n\n- [ ] \n- [ ] \n\n#### 学习/工作\n\n- [ ] \n- [ ] \n\n#### 生活/其他\n\n- [ ] \n- [ ] \n\n#### 随手小记\n\n- \n- \n";

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
    fs::read_to_string(&md_path).map_err(|e| e.to_string())
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

/// A module is complete when every checkbox is checked, or when it has no
/// checkboxes at all (empty module counts as complete).
fn section_complete(section: &str) -> bool {
    let boxes: Vec<&str> = section
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("- [") || l.starts_with("* ["))
        .collect();
    if boxes.is_empty() {
        return true;
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

struct SettleOutcome {
    id: String,
    reward: u32,
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

    notes[target_idx].settled = true;
    notes[target_idx].triple_complete = triple;
    write_index(app, &notes);

    let mut config = load_config(app);
    config.yuan_shi += reward;
    save_config(app, &config);

    Some(SettleOutcome {
        id: target_id,
        reward,
        triple,
    })
}

// ── Auto-create today's commission ──

/// Build today's content by inheriting from a previous commission: 必做 carries
/// every item (reset to unchecked), other settled modules carry only unchecked
/// items, and 随手小记 stays blank.
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
                    let mut inserted = false;
                    if let Some(prev_sec) = prev_sections.get(sec_name) {
                        let is_checkbox = |l: &&str| -> bool {
                            let t = l.trim_start();
                            t.starts_with("- [") || t.starts_with("* [")
                        };
                        let is_unchecked = |l: &&str| -> bool {
                            let t = l.trim_start();
                            t.starts_with("- [ ]") || t.starts_with("* [ ]")
                        };
                        let raw_items: Vec<&str> = if sec_name == "必做" {
                            prev_sec.lines().filter(is_checkbox).collect()
                        } else {
                            prev_sec.lines().filter(is_unchecked).collect()
                        };
                        if !raw_items.is_empty() {
                            for item in &raw_items {
                                let line = if sec_name == "必做" {
                                    reset_checkbox(item)
                                } else {
                                    item.to_string()
                                };
                                result.push_str(&line);
                                result.push('\n');
                            }
                            inserted = true;
                        }
                    }
                    if !inserted {
                        result.push_str("- [ ] \n");
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
    days: Vec<WeeklyDay>,
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

    let mut config = load_config(&app);
    let (eligible, reward) = if count >= 5 {
        let reward = 100 * (config.streak + 1);
        config.streak += 1;
        if config.streak == 7 {
            config.streak = 0;
        }
        config.yuan_shi += reward;
        (true, reward)
    } else {
        config.streak = 0;
        (false, 0)
    };
    save_config(&app, &config);

    WeeklyResult {
        count,
        eligible,
        reward,
        streak: config.streak,
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
    dark_mode: bool,
    opacity: f32,
    #[serde(default)]
    yuan_shi: u32,
    #[serde(default)]
    streak: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            dark_mode: false,
            opacity: 0.92,
            yuan_shi: 0,
            streak: 0,
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
    if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        AppConfig::default()
    }
}

fn save_config(app: &tauri::AppHandle, config: &AppConfig) {
    let path = config_path(app);
    let data = serde_json::to_string_pretty(config).unwrap_or_default();
    fs::write(&path, data).ok();
}

#[tauri::command]
fn get_yuan_shi(app: tauri::AppHandle) -> u32 {
    load_config(&app).yuan_shi
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
fn get_dark_mode(app: tauri::AppHandle) -> bool {
    load_config(&app).dark_mode
}

#[tauri::command]
fn set_dark_mode(app: tauri::AppHandle, dark_mode: bool) {
    let mut config = load_config(&app);
    config.dark_mode = dark_mode;
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

// ── Update check ──

const GITEE_OWNER: &str = "Honest_Leon";
const GITEE_REPO: &str = "lightweight-sticky-notes";

#[derive(Debug, Serialize, Clone)]
struct UpdateInfo {
    current_version: String,
    latest_version: String,
    has_update: bool,
    download_url: String,
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect()
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

#[tauri::command]
fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let url = format!(
        "https://gitee.com/{}/{}/raw/master/src-tauri/tauri.conf.json",
        GITEE_OWNER, GITEE_REPO
    );

    let body = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| format!("检查更新失败：无法连接 gitee（{}）", e))?
        .into_string()
        .map_err(|e| format!("读取响应失败：{}", e))?;

    let conf: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("解析远程版本失败：{}", e))?;
    let latest = conf
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or(&current)
        .to_string();

    let has_update = version_gt(&latest, &current);
    let filename = format!("轻量便签_{}_x64-setup.exe", latest);
    let download_url = format!(
        "https://gitee.com/{}/{}/raw/master/releases/{}",
        GITEE_OWNER,
        GITEE_REPO,
        percent_encode(&filename)
    );

    Ok(UpdateInfo {
        current_version: current,
        latest_version: latest,
        has_update,
        download_url,
    })
}

#[tauri::command]
fn download_and_install(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let tmp = std::env::temp_dir().join("lwsn-update");
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let installer = tmp.join("setup.exe");

    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|e| format!("下载失败：{}", e))?;
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&installer).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| format!("保存安装包失败：{}", e))?;

    let status = std::process::Command::new(&installer)
        .arg("/S")
        .status()
        .map_err(|e| format!("启动安装器失败：{}", e))?;
    if !status.success() {
        return Err("安装未成功，请前往 gitee 手动下载安装".to_string());
    }

    let exe_name = "lightweight-sticky-notes.exe";
    let candidates: Vec<PathBuf> = std::env::var("LOCALAPPDATA")
        .map(|base| {
            let base = PathBuf::from(base);
            vec![
                base.join("轻量便签").join(exe_name),
                base.join("Programs").join("LightweightStickyNotes").join(exe_name),
                base.join("LightweightStickyNotes").join(exe_name),
            ]
        })
        .unwrap_or_default();

    if let Some(new_exe) = candidates.iter().find(|p| p.exists()) {
        std::process::Command::new(new_exe).spawn().ok();
        std::thread::sleep(std::time::Duration::from_millis(800));
        app.exit(0);
    }

    Ok(())
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

fn build_tray(app: &tauri::App) -> Result<tauri::tray::TrayIcon, Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let icon = app.default_window_icon().unwrap().clone();

    let autostart_enabled = is_autostart_enabled_internal();
    let autostart_label = if autostart_enabled {
        "开机自启：开"
    } else {
        "开机自启：关"
    };

    let show = MenuItemBuilder::with_id("show", "显示窗口").build(app)?;
    let autostart = MenuItemBuilder::with_id("autostart", autostart_label).build(app)?;
    let autostart_clone = autostart.clone();
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&show, &autostart, &separator, &quit])
        .build()?;

    let tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("轻量便签")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "show" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "autostart" => {
                    let new_state =
                        toggle_autostart_internal().unwrap_or_else(|_| is_autostart_enabled_internal());
                    let label = if new_state {
                        "开机自启：开"
                    } else {
                        "开机自启：关"
                    };
                    let _ = autostart_clone.set_text(label);
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
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
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
        // Empty -> complete (user's rule)
        assert!(section_complete(""));
        // Non-checkbox text only -> complete
        assert!(section_complete("just some notes\n"));
        // Asterisk markers also count
        assert!(section_complete("* [x] a\n* [x] b\n"));
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
    fn percent_encoding_keeps_ascii_and_encodes_unicode() {
        assert_eq!(percent_encode("setup_0.1.1.exe"), "setup_0.1.1.exe");
        assert!(percent_encode("轻量便签").contains('%'));
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            list_notes,
            delete_note,
            rename_note,
            save_note,
            load_note,
            init_daily,
            check_weekly,
            get_yuan_shi,
            get_streak,
            get_close_behavior,
            set_close_behavior,
            enable_autostart_cmd,
            disable_autostart_cmd,
            is_autostart_enabled_cmd,
            cleanup_old_autostart_cmd,
            get_dark_mode,
            set_dark_mode,
            get_opacity,
            set_opacity,
            check_update,
            download_and_install,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            migrate_autostart_entry();
            cleanup_old_autostart(&app_handle);

            let _tray = build_tray(app)?;

            let window = app.get_webview_window("main").unwrap();
            let w_clone = window.clone();

            if !std::env::args().any(|a| a == "--startup") {
                let _ = window.show();
            }

            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let config = load_config(&app_handle);
                    if config.close_to_tray {
                        api.prevent_close();
                        let _ = w_clone.hide();
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
