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
/// 桌宠「今天填了没」的提醒只看这两个模块。
/// 与前端 `tomorrowCheck.ts` 的 TOMORROW_ALLOWED_SECTIONS 是同一份字符串的第二副本。
const TOMORROW_MODULES: [&str; 2] = ["学习/工作", "生活/其他"];

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

fn read_index_file(path: &PathBuf) -> Option<Vec<NoteMeta>> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn index_bak_path(path: &PathBuf) -> PathBuf {
    path.with_file_name("notes_index.json.bak")
}

/// 索引必须和配置一样原子写：截断的 JSON 会让 read_index 走“空数组”分支，
/// 随后自动创建今天的委托时用一条新记录覆盖掉整段历史。
fn write_index_no_backup(path: &PathBuf, data: &str) -> std::io::Result<()> {
    let tmp = path.with_file_name("notes_index.json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(data.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

fn write_index_atomic(path: &PathBuf, data: &str) -> std::io::Result<()> {
    let bak = index_bak_path(path);
    if path.exists() {
        let _ = fs::rename(path, &bak);
    }
    write_index_no_backup(path, data)
}

fn write_index(app: &tauri::AppHandle, notes: &[NoteMeta]) -> bool {
    let path = index_path(app);
    let data = serde_json::to_string_pretty(notes).unwrap_or_default();
    match write_index_atomic(&path, &data) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("写入 notes_index.json 失败: {e}");
            false
        }
    }
}

fn system_time_to_local(t: std::time::SystemTime) -> String {
    let dt: chrono::DateTime<Local> = t.into();
    dt.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// 索引与 .bak 都读不出来时，从 notes/*.md 重建一份最小索引。
/// 旧便签一律视为已结算：宁可少补一次历史奖励，也不能让损坏的索引导致重复发奖。
fn rebuild_index(app: &tauri::AppHandle) -> Vec<NoteMeta> {
    let dir = notes_dir(app);
    let mut notes = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return notes;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let meta = fs::metadata(&path).ok();
        let created = meta
            .as_ref()
            .and_then(|m| m.created().ok())
            .or_else(|| meta.as_ref().and_then(|m| m.modified().ok()));
        let updated = meta.as_ref().and_then(|m| m.modified().ok());
        let created_at = created
            .map(system_time_to_local)
            .unwrap_or_else(|| String::from("1970-01-01T00:00:00"));
        let updated_at = updated.map(system_time_to_local).unwrap_or_else(|| created_at.clone());
        let is_daily = content.contains("### 每日委托");
        notes.push(NoteMeta {
            id: id.to_string(),
            title: if is_daily {
                format_daily_title(&created_at)
            } else {
                id.chars().take(8).collect()
            },
            created_at: created_at.clone(),
            updated_at,
            title_edited: false,
            settled: !is_today(&created_at),
            triple_complete: settle_reward(&parse_md_sections(&content)).1,
        });
    }
    notes.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    notes
}

fn read_index(app: &tauri::AppHandle) -> Vec<NoteMeta> {
    let path = index_path(app);
    if let Some(notes) = read_index_file(&path) {
        return notes;
    }
    if let Some(notes) = read_index_file(&index_bak_path(&path)) {
        // 主文件坏了但备份还在：把备份转正，避免每次启动都从这里兜底。
        // 不能走 write_index_atomic——那会把损坏的主文件 rotate 成 .bak，
        // 把刚刚救回来的好备份覆盖掉。
        let data = serde_json::to_string_pretty(&notes).unwrap_or_default();
        let _ = fs::remove_file(&path);
        let _ = write_index_no_backup(&path, &data);
        return notes;
    }
    let rebuilt = rebuild_index(app);
    if !rebuilt.is_empty() {
        write_index(app, &rebuilt);
    }
    rebuilt
}

/// 委托正文也走临时文件 + rename，进程在写一半时被杀不会留下截断的 .md。
fn write_note_atomic(path: &PathBuf, data: &[u8]) -> std::io::Result<()> {
    let mut tmp_name = path.as_os_str().to_os_string();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
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
    write_note_atomic(&md_path, content.as_bytes()).map_err(|e| e.to_string())?;
    let mut notes = read_index(app);
    // 索引损坏时 read_index 会从 notes/*.md 重建，而新文件刚刚已经写进去了；
    // 这时直接 push 会出现两条同 id 记录，所以先找再替换。
    if let Some(existing) = notes.iter_mut().find(|n| n.id == note.id) {
        *existing = note.clone();
    } else {
        notes.push(note.clone());
    }
    if !write_index(app, &notes) {
        // 索引没写成就不留半张看不见的便签
        let _ = fs::remove_file(&md_path);
        return Err("保存委托索引失败".to_string());
    }
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
    match (iso.get(5..7), iso.get(8..10)) {
        (Some(month), Some(day)) => format!("{month}{day}"),
        _ => String::new(),
    }
}

fn is_day(iso: &str, day: &str) -> bool {
    iso.get(..10) == Some(day)
}

fn is_today(iso: &str) -> bool {
    is_day(iso, &today_ymd())
}

/// 选最近一份“早于今天”的每日委托。
/// 必须按 created_at 而不是 updated_at：用户打开旧便签改一个字就会刷新
/// updated_at，按它选会把任意旧便签当成“上一份委托”。
/// only_unsettled 供结算使用；继承要拿已经结算过的昨天那份，所以不能过滤 settled。
fn latest_daily_before(
    notes: &[NoteMeta],
    today: &str,
    only_unsettled: bool,
    mut load: impl FnMut(&NoteMeta) -> Option<String>,
) -> Option<(usize, String)> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            n.created_at
                .get(..10)
                .is_some_and(|date| date < today)
                && (!only_unsettled || !n.settled)
        })
        .filter_map(|(i, n)| {
            let content = load(n)?;
            content.contains("### 每日委托").then_some((i, content))
        })
        .max_by(|a, b| notes[a.0].created_at.cmp(&notes[b.0].created_at))
}

fn parse_date(iso: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(iso.get(..10)?, "%Y-%m-%d").ok()
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
    boxes.iter().all(|l| is_checked_item(l))
}

/// 已勾选的任务行（空白占位不算，见 `is_blank_item`）
fn is_checked_item(line: &str) -> bool {
    let l = line.trim_start();
    l.starts_with("- [x]")
        || l.starts_with("- [X]")
        || l.starts_with("* [x]")
        || l.starts_with("* [X]")
}

/// 小节里是否填了真实内容：非空行都算，但模板留下的空白占位（`- [ ] `、裸 `-`）不算。
/// 代码围栏已在 `parse_md_sections` 整段剔除，不会混进来。
fn section_has_content(section: &str) -> bool {
    section.lines().any(|l| match checkbox_tail(l) {
        // 复选框行看标记后还有没有文字（模板的空白占位不算填了）
        Some(tail) => !tail.trim().is_empty(),
        // 其余行去掉列表符与引用符后还剩字，才算内容
        None => !l.trim()
            .trim_start_matches(['-', '*', '+', '>', ' '])
            .trim()
            .is_empty(),
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

    // 最近一份“早于今天、未结算、确实是每日委托”的便签，按 created_at 选。
    let (target_idx, content) = latest_daily_before(&notes, &today_ymd(), true, |n| {
        fs::read_to_string(notes_dir(app).join(format!("{}.md", &n.id))).ok()
    })?;

    let target_id = notes[target_idx].id.clone();
    let sections = parse_md_sections(&content);
    let (reward, triple) = settle_reward(&sections);
    let mora = mora_reward(&sections);

    notes[target_idx].settled = true;
    notes[target_idx].triple_complete = triple;
    if !write_index(app, &notes) {
        // 索引没落盘：保持未结算，下次启动重试
        return None;
    }

    if reward > 0 || mora > 0 {
        let mut config = load_config(app);
        config.yuan_shi += reward;
        config.mora += mora;
        if !save_config(app, &config) {
            // 余额没落盘：把 settled 标记回滚，避免下次启动漏发这次奖励
            notes[target_idx].settled = false;
            notes[target_idx].triple_complete = false;
            if !write_index(app, &notes) {
                eprintln!("回滚 {} 的 settled 标记失败，本次奖励可能丢失", target_id);
            }
            return None;
        }
    }

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
                        let is_unchecked = |l: &str| -> bool {
                            let t = l.trim_start();
                            t.starts_with("- [ ]") || t.starts_with("* [ ]")
                        };
                        // 普通待办：必做全量继承并重置勾选，其余只继承未完成项；
                        // 空白项（空委托）一律不继承。
                        // 一个顶层任务连同其缩进续行/嵌套子项作为一个 block 一起复制，
                        // 否则“- [ ] 主任务”下面的说明和子项会在新的一天消失。
                        let lines: Vec<&str> = prev_sec.lines().collect();
                        let mut i = 0;
                        while i < lines.len() {
                            let line = lines[i];
                            let trimmed = line.trim_start();
                            if line.len() != trimmed.len() {
                                // 前面没有顶层任务认领的缩进行，丢掉
                                i += 1;
                                continue;
                            }
                            let mut block: Vec<&str> = vec![line];
                            let mut j = i + 1;
                            while j < lines.len() {
                                let next = lines[j];
                                let next_trimmed = next.trim_start();
                                if next_trimmed.is_empty() || next.len() != next_trimmed.len() {
                                    block.push(next);
                                    j += 1;
                                } else {
                                    break;
                                }
                            }
                            while block.len() > 1
                                && block.last().is_some_and(|l| l.trim().is_empty())
                            {
                                block.pop();
                            }
                            let is_task =
                                trimmed.starts_with("- [") || trimmed.starts_with("* [");
                            if is_task && !is_blank_item(line) {
                                if sec_name == "必做" || is_unchecked(line) {
                                    for item in &block {
                                        let item_trimmed = item.trim_start();
                                        let item_is_task = item_trimmed.starts_with("- [")
                                            || item_trimmed.starts_with("* [");
                                        // 必做要求全量重置勾选；嵌套子任务也一起重置
                                        if sec_name == "必做" && item_is_task {
                                            items.push(reset_checkbox(item));
                                        } else {
                                            items.push((*item).to_string());
                                        }
                                    }
                                } else {
                                    // 顶层任务已完成，但嵌套子项里可能还有未完成项：
                                    // 保持旧的逐行继承行为，只带出未完成的缩进子任务。
                                    for item in &block[1..] {
                                        let item_trimmed = item.trim_start();
                                        let indented = item.len() != item_trimmed.len();
                                        if indented
                                            && !is_blank_item(item)
                                            && (item_trimmed.starts_with("- [ ]")
                                                || item_trimmed.starts_with("* [ ]"))
                                        {
                                            items.push((*item).to_string());
                                        }
                                    }
                                }
                            } else if !is_task {
                                // 顶层不是任务（普通列表项/引用等）：旧行为仍会带出块内
                                // 未完成的嵌套任务，这里保持
                                for item in &block[1..] {
                                    let item_trimmed = item.trim_start();
                                    let indented = item.len() != item_trimmed.len();
                                    if indented
                                        && !is_blank_item(item)
                                        && (item_trimmed.starts_with("- [ ]")
                                            || item_trimmed.starts_with("* [ ]"))
                                    {
                                        items.push((*item).to_string());
                                    }
                                }
                            }
                            i = j;
                        }
                        // 明日委托（引用块 `> text`）：在新一天转为普通任务 `- [ ] text`。
                        // 只处理顶层引用；缩进引用属于上方任务的块内容，已在 block 里复制，
                        // 再转换一次会重复。
                        for pline in prev_sec.lines() {
                            if pline.len() != pline.trim_start().len() {
                                continue;
                            }
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
    // 继承同样按 created_at 取最近一份每日委托；编辑旧便签不能改变“昨天”是谁。
    // 这里不能过滤 settled：昨天的委托在启动时通常已经结算过了，但仍然是继承来源。
    let previous = latest_daily_before(&notes, &today_ymd(), false, |n| {
        fs::read_to_string(notes_dir(app).join(format!("{}.md", &n.id))).ok()
    });

    match previous {
        Some((_, prev)) => inherit_from_prev(&prev),
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
    // fenced code block 里的 #### 与 - [x] 都是示例，不是委托结构。
    // 整段跳过（而非只跳过围栏行），结算/继承才不会把代码当任务。
    let mut fence: Option<&'static str> = None;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some("```");
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some("~~~");
            continue;
        }
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
/// - `already = false` 且达标：发奖 100*(base+1)，连续周数 +1（满 7 周清零）。
/// - `already = false` 且未达标：只在“上一周也没发奖”时把连击清零；
///   周中查看不能提前判负，否则周一打开一次就把连续周数清掉了。
fn weekly_settle_logic(
    count: u32,
    streak: u32,
    already: bool,
    prev_week_awarded: bool,
) -> (bool, u32, u32) {
    let eligible = count >= 5;
    if already {
        return (eligible, 0, streak);
    }
    let base_streak = if prev_week_awarded { streak } else { 0 };
    if eligible {
        let mut new_streak = base_streak + 1;
        if new_streak == 7 {
            new_streak = 0;
        }
        (true, 100 * (base_streak + 1), new_streak)
    } else {
        (false, 0, base_streak)
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

    let monday_str = monday.format("%Y-%m-%d").to_string();
    let prev_monday = (monday - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let mut config = load_config(&app);
    let already = config.weekly_settled_week == monday_str;
    // 只有“上一自然周确实发过奖”才延续连击；跳过整周、或上一周未达标都会在
    // 新一周的第一次查看时中断。周中查看本身不会清零（见 weekly_settle_logic）。
    let prev_week_awarded = config.weekly_settled_week == prev_monday;
    let (eligible, reward, new_streak) =
        weekly_settle_logic(count, config.streak, already, prev_week_awarded);
    if !already {
        config.streak = new_streak;
        config.yuan_shi += reward;
        // 方案 B：只有“达标发奖”才锁定本周（防止同周重复发奖）；
        // 未达标不锁定，本周后续补达标仍可获得奖励。
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
    // 先更新索引再删文件：索引写失败时便签还完整可见，不会留下“列表里有、文件没了”的死条目
    let mut notes = read_index(&app);
    notes.retain(|n| n.id != id);
    if !write_index(&app, &notes) {
        return Err("保存委托索引失败".to_string());
    }
    let md_path = notes_dir(&app).join(format!("{}.md", &id));
    if let Err(e) = fs::remove_file(&md_path) {
        eprintln!("删除便签文件失败（索引已移除）: {e}");
    }
    Ok(())
}

#[tauri::command]
fn save_note(app: tauri::AppHandle, id: String, content: String) -> Result<(), String> {
    use tauri::Emitter;
    let md_path = notes_dir(&app).join(format!("{}.md", &id));
    // 空复选框补尾空格，保证保存/重开后仍能渲染
    let content = ensure_task_trailing_space(&content);
    let existing = fs::read_to_string(&md_path).ok();

    // 内容未变化则跳过：编辑器加载时 markdownUpdated 会回传一次原内容，
    // 没必要为同一份内容再做一次原子写与 fsync。
    if existing.as_deref() == Some(content.as_str()) {
        return Ok(());
    }

    write_note_atomic(&md_path, content.as_bytes()).map_err(|e| e.to_string())?;

    // 勾掉一条委托 = 已完成条数变大，当场广播。用事件而不是让前端事后比较计数：
    // 便签态不渲染桌宠组件，「等前端入场再推算增量」必然要和保存/入场两条异步路径抢顺序。
    let before = existing.map(|c| progress_of(&c).checked_count).unwrap_or_default();
    let after = progress_of(&content).checked_count;
    if after > before {
        let _ = app.emit("task-completed", after - before);
    }

    let mut notes = read_index(&app);
    let mut index_changed = false;
    if let Some(note) = notes.iter_mut().find(|n| n.id == id) {
        // Title reflects the commission's own day (created_at), never the
        // editing time — editing yesterday's note must not retitle it.
        // updated_at 不再参与结算/继承（已改按 created_at），所以正常保存不重写索引，
        // 避免每 500ms 多一次临时文件 + fsync；只有标题确实需要修复时才落索引。
        let title = if note.title_edited {
            note.title.clone()
        } else {
            format_daily_title(&note.created_at)
        };
        if note.title != title {
            note.title = title;
            index_changed = true;
        }
    }
    if index_changed && !write_index(&app, &notes) {
        return Err("保存委托索引失败".to_string());
    }
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
    if !write_index(&app, &notes) {
        return Err("保存委托索引失败".to_string());
    }
    Ok(())
}

#[tauri::command]
fn load_note(app: tauri::AppHandle, id: String) -> Result<String, String> {
    load_note_internal(&app, &id)
}

// ── App config ──

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppConfig {
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
            opacity: 0.92,
            yuan_shi: 0,
            mora: 0,
            streak: 0,
            weekly_settled_week: String::new(),
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

fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> bool {
    let path = config_path(app);
    let data = serde_json::to_string_pretty(config).unwrap_or_default();
    match write_config_atomic(&path, &data) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("写入 config.json 失败: {e}");
            false
        }
    }
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
    /// 三个结算模块里已勾选的非空委托条数。`save_note` 比较新旧两份正文的这个数，
    /// 变大就广播 `task-completed`，桌宠据此演金袋。
    checked_count: u32,
    /// 学习/工作、生活/其他 两个模块里是否填了内容（桌宠提醒气泡的依据）
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
            checked_count: 0,
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
    let items = || {
        SETTLE_MODULES
            .iter()
            .filter_map(|m| sections.get(*m))
            .flat_map(|s| s.lines().map(str::trim_start))
            .filter(|l| (l.starts_with("- [") || l.starts_with("* [")) && !is_blank_item(l))
    };
    let has_tasks = items().next().is_some();
    let checked_count = items().filter(|l| is_checked_item(l)).count() as u32;
    let tomorrow_filled = TOMORROW_MODULES
        .iter()
        .filter_map(|m| sections.get(*m))
        .any(|s| section_has_content(s));
    TodayProgress {
        modules_done: done,
        triple: done == SETTLE_MODULES.len() as u32,
        has_tasks,
        checked_count,
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

/// 鼠标左键此刻是否被按着（全局状态，不看焦点）。便签的拖动与拉边框都走系统模态循环，
/// WebView 里那些 pointerdown/buttons 一个都收不到，前端只能靠这个判断「鼠标离开窗口」
/// 到底是移开了还是正拿着这扇窗。
#[tauri::command]
fn is_mouse_button_down() -> bool {
    mouse_button_down_internal()
}

/// 光标此刻是否还在主窗口的**外框**矩形内。前端只看得见客户区：指针移到窗口那圈隐形的
/// 可拉伸边框上（拉边框全靠它）时 WebView 已经发了 pointerleave，但人根本没离开窗口。
/// 便签据此缩回就成了「一碰边框就变宠物」。读不到一律 false = 按离开处理，行为退回旧版。
#[tauri::command]
fn is_cursor_on_window(app: tauri::AppHandle) -> bool {
    let Some(w) = app.get_webview_window("main") else {
        return false;
    };
    cursor_on_window_internal(&w)
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

/// 带标题栏/可拉伸边框的最大化窗口是普通应用（副屏没有任务栏时它的矩形也会盖满
/// 显示器），不是全屏游戏/投屏。无边框的最大化窗口仍按全屏处理。
#[cfg(windows)]
fn is_ordinary_maximized(is_zoomed: bool, style: u32) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{WS_CAPTION, WS_THICKFRAME};
    is_zoomed && (style & WS_CAPTION.0 != 0 || style & WS_THICKFRAME.0 != 0)
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

/// 鼠标左键此刻是否被按着（全局状态，不看焦点、不看光标在哪）。便签的拖动与拉边框都走
/// 系统模态循环：那期间 WebView 一个 pointer 事件都收不到，`e.buttons` 与 pointerdown
/// 全都指不上，只有这个能说明「鼠标离开窗口」其实是「用户正拿着这扇窗」。
/// 拉边框前先把指针停在边框上，那一刻的 pointerleave 是空手离开的（buttons=0），
/// 所以前端光看事件挡不住，必须回来问 OS。
#[cfg(windows)]
fn mouse_button_down_internal() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    // 高位=此刻按下，低位=自上次查询以来按过（会把历史上的单击也算进来）——只取高位
    let state = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) };
    (state as u16) & 0x8000 != 0
}

#[cfg(not(windows))]
fn mouse_button_down_internal() -> bool {
    false
}

/// 光标是否落在矩形内。右下两边取开区间，跟 GetWindowRect 的「right/bottom 是边界外
/// 第一个像素」的语义对齐，否则贴着边会两头都算在内。
fn point_in_rect(pt: (i32, i32), rect: [i32; 4]) -> bool {
    pt.0 >= rect[0] && pt.0 < rect[2] && pt.1 >= rect[1] && pt.1 < rect[3]
}

#[cfg(windows)]
fn cursor_on_window_internal(window: &tauri::WebviewWindow) -> bool {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};

    let Ok(hwnd) = window.hwnd() else {
        return false;
    };
    let mut pt = POINT::default();
    if unsafe { GetCursorPos(&mut pt) }.is_err() {
        return false;
    }
    let mut rc = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rc) }.is_err() {
        return false;
    }
    point_in_rect((pt.x, pt.y), [rc.left, rc.top, rc.right, rc.bottom])
}

#[cfg(not(windows))]
fn cursor_on_window_internal(_window: &tauri::WebviewWindow) -> bool {
    false
}

#[cfg(windows)]
fn foreground_fullscreen_internal(app: &tauri::AppHandle) -> bool {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect, IsZoomed, GWL_STYLE,
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

    // 副屏通常没有任务栏，普通最大化窗口的矩形会盖满整块显示器；不要把
    // 浏览器/编辑器最大化误判成“他人全屏”。
    let is_zoomed = unsafe { IsZoomed(hwnd) }.as_bool();
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    if is_ordinary_maximized(is_zoomed, style) {
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

/// 发布物托管在 GitHub（仓库名与 Gitee 时代的 lightweight-sticky-notes 不同，
/// 别按旧名拼 URL）。更新检查与标题栏的仓库链接都以这两个常量为准。
const GITHUB_OWNER: &str = "HonestLeon";
const GITHUB_REPO: &str = "Daily-Commissions-Simulator";

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

/// 更新检查的内存缓存：命中且未过期则直接返回，避免每次点击都请求 GitHub、
/// 触发匿名接口限流（GitHub 匿名额度按 IP 计，每小时只有 60 次）。
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

/// 通过 GitHub Release API 检查更新：tag_name 作为最新版本号，
/// 从附件中选取安装包下载地址。结果带 TTL 缓存以降低撞限流概率。
/// 网络请求是阻塞的 ureq：用 async 命令 + spawn_blocking，别卡住窗口主线程。
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(move || check_update_sync(&app))
        .await
        .map_err(|e| format!("检查更新任务失败：{e}"))?
}

fn check_update_sync(app: &tauri::AppHandle) -> Result<UpdateInfo, String> {
    // 命中有效缓存则直接返回（含失败缓存，实现短冷却）
    if let Some(cached) = update_cache().lock().unwrap().clone() {
        if cache_fresh(cached.at, cached.result.is_ok()) {
            return cached.result;
        }
    }

    let fetched = fetch_update_info(app);
    *update_cache().lock().unwrap() = Some(UpdateCache {
        at: Instant::now(),
        result: fetched.clone(),
    });
    fetched
}

  /// 实际向 GitHub 发起请求并解析版本信息（不缓存）。
  fn fetch_update_info(app: &tauri::AppHandle) -> Result<UpdateInfo, String> {
      let current = app.package_info().version.to_string();
      let url = format!(
          "https://api.github.com/repos/{}/{}/releases/latest",
        GITHUB_OWNER, GITHUB_REPO
      );

      let resp = match ureq::get(&url)
          // GitHub API 缺 User-Agent 会被直接 403；Accept 钉住版本，免得默认值随 ureq 变化
          .set("User-Agent", "Daily-Commissions-Simulator-Updater")
          .set("Accept", "application/vnd.github+json")
          .timeout(std::time::Duration::from_secs(15))
          .call()
      {
          Ok(resp) => resp,
          Err(ureq::Error::Status(404, _)) => {
              return Err("检查更新失败：仓库尚未发布任何 Release".to_string());
          }
          // 匿名接口按 IP 限流，403 多半是撞额度了，别报成「连不上」
          Err(ureq::Error::Status(code, _)) => {
              return Err(format!("检查更新失败：GitHub 返回 {code}，请稍后重试"));
          }
          Err(e) => return Err(format!("检查更新失败：无法连接 github（{}）", e)),
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
                "最新 Release 未附带安装包（.exe），请前往 GitHub 发布页手动下载".to_string(),
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
timeout /t 3 /nobreak >nul
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

/// 安装完成后延迟 3 秒拉起新版本：先让当前进程退出、释放单实例互斥量，
/// 否则新进程会在旧进程还活着时启动，被单实例检查判成第二实例而直接退出。
#[cfg(windows)]
fn restart_after_exit(exe: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    let dir = std::env::temp_dir().join("lwsn-update");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let bat = dir.join("restart.bat");
    let script =
        "@echo off\r\nif \"%~1\"==\"\" exit /b 1\r\ntimeout /t 3 /nobreak >nul\r\nstart \"\" \"%~1\"\r\nexit /b 0\r\n";
    fs::write(&bat, script).map_err(|e| format!("写入重启脚本失败：{}", e))?;
    std::process::Command::new("cmd")
        .args(["/c", &bat.to_string_lossy(), &exe.to_string_lossy()])
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("启动重启脚本失败：{}", e))?;
    Ok(())
}

#[cfg(not(windows))]
fn restart_after_exit(exe: &std::path::Path) -> Result<(), String> {
    std::process::Command::new(exe)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动新版本失败：{e}"))
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
        return Err("安装未成功，请前往 GitHub 发布页手动下载安装".to_string());
    }

    let exe_name = "lightweight-sticky-notes.exe";
    let candidates: Vec<PathBuf> = std::env::var("LOCALAPPDATA")
        .map(|base| installed_dirs(&PathBuf::from(base)))
        .unwrap_or_default()
        .into_iter()
        .map(|d| d.join(exe_name))
        .collect();

    let Some(new_exe) = candidates.iter().find(|p| p.exists()) else {
        return Err("安装完成，但未找到新版本程序，请手动启动".to_string());
    };
    restart_after_exit(new_exe)?;
    // 给 cmd 一点时间接住脚本；真正的启动延迟在 bat 里（等本进程退出）
    std::thread::sleep(std::time::Duration::from_millis(200));
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn download_and_install(
    app: tauri::AppHandle,
    setup_url: String,
    portable_url: String,
) -> Result<(), String> {
    // 下载最多 600s、安装器还要同步等待；放到阻塞线程池，主窗口保持可交互
    tauri::async_runtime::spawn_blocking(move || {
        download_and_install_sync(&app, &setup_url, &portable_url)
    })
    .await
    .map_err(|e| format!("更新任务失败：{e}"))?
}

fn download_and_install_sync(
    app: &tauri::AppHandle,
    setup_url: &str,
    portable_url: &str,
) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let is_installed = std::env::var("LOCALAPPDATA")
        .map(|base| exe_is_installed_version(&exe, &PathBuf::from(base)))
        .unwrap_or(false);

    if is_installed {
        if setup_url.is_empty() {
            return Err("最新 Release 未附带安装版安装包，请前往 GitHub 发布页手动下载"
                .to_string());
        }
        install_via_setup(app, setup_url)
    } else {
        if portable_url.is_empty() {
            return Err("最新 Release 未附带免安装版程序，请前往 GitHub 发布页手动下载"
                .to_string());
        }
        self_replace_portable(app, portable_url, &exe)
    }
}

// ── Autostart via registry (reliable, no VBS) ──

fn autostart_reg_key() -> &'static str {
    r"Software\Microsoft\Windows\CurrentVersion\Run"
}

fn autostart_reg_value() -> &'static str {
    "LightweightStickyNotes"
}

/// 开机自启只写可执行路径，不带参数：曾经附带的 `--startup` 表示「启动即藏进托盘」，
/// 现在自启也照常显示窗口，那个开关已经没有读取方了。
fn autostart_reg_value_data(exe_path: &str) -> String {
    format!("\"{}\"", exe_path)
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

/// 从注册表里现存的自启数据中剥出可执行路径。早期写成 `"<exe>" --startup`
/// （`--startup` = 启动即藏进托盘），现在自启也直接显示窗口，那个开关已经没有读取方。
fn autostart_exe_path(value_data: &str) -> String {
    let raw = value_data.trim();
    let body = match raw.split_once("--startup") {
        Some((head, _)) => head,
        None => raw,
    };
    body.trim().trim_matches('"').trim().to_string()
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
                let exe_path = autostart_exe_path(&old);
                if !exe_path.is_empty() && std::path::Path::new(&exe_path).exists() {
                    let _ = key.set_value(
                        autostart_reg_value(),
                        &autostart_reg_value_data(&exe_path),
                    );
                }
                let _ = key.delete_value("lightweight-sticky-notes");
            }
            // 本值名下的数据还带着没人读的开关时改写成规范形式，否则老装机永远留着它
            if let Ok(cur) = key.get_value::<String, _>(autostart_reg_value()) {
                let exe_path = autostart_exe_path(&cur);
                let want = autostart_reg_value_data(&exe_path);
                if cur != want && !exe_path.is_empty() && std::path::Path::new(&exe_path).exists() {
                    let _ = key.set_value(autostart_reg_value(), &want);
                }
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
fn enable_autostart_cmd(app: tauri::AppHandle) -> Result<bool, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let enabled = enable_autostart_in_registry(&exe.to_string_lossy())?;
    if enabled {
        refresh_tray_menu(&app);
    }
    Ok(enabled)
}

#[tauri::command]
fn disable_autostart_cmd(app: tauri::AppHandle) -> Result<bool, String> {
    disable_autostart_in_registry();
    refresh_tray_menu(&app);
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

/// 双开保护的结果。Primary 携带“显示窗口”命名事件的句柄（Windows）；
/// Secondary 表示已有实例在跑，当前进程应直接退出。
enum InstanceRole {
    Primary(Option<isize>),
    Secondary,
}

/// 用命名互斥量判断是否已有实例，并用命名事件通知已有实例把窗口叫出来。
/// 不用 tauri-plugin-single-instance，避免为一个小功能新增依赖；命名对象由
/// Local 命名空间隔离到当前登录会话，和本应用的单用户数据目录语义一致。
#[cfg(windows)]
fn claim_single_instance() -> InstanceRole {
    use windows::core::w;
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, SetLastError, ERROR_ALREADY_EXISTS, ERROR_SUCCESS,
    };
    use windows::Win32::System::Threading::{CreateEventW, CreateMutexW, SetEvent};

    unsafe {
        let event = match CreateEventW(None, true, false, w!("Local\\daily-commissions-show")) {
            Ok(handle) => handle,
            Err(_) => return InstanceRole::Primary(None),
        };
        // CreateEventW 遇到已存在的命名事件也会把 last error 设成 ERROR_ALREADY_EXISTS；
        // 必须先清零，否则下一次 CreateMutexW 成功新建时也会被误判成“已有实例”。
        SetLastError(ERROR_SUCCESS);
        let mutex = match CreateMutexW(None, true, w!("Local\\daily-commissions-instance")) {
            Ok(handle) => handle,
            Err(_) => {
                let _ = CloseHandle(event);
                return InstanceRole::Primary(None);
            }
        };
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = SetEvent(event);
            let _ = CloseHandle(event);
            let _ = CloseHandle(mutex);
            return InstanceRole::Secondary;
        }
        // 互斥量句柄不关闭：它要活到进程退出，否则命名对象消失，双开保护会失效。
        // HANDLE 没有 Drop，离开作用域不会自动 CloseHandle。
        InstanceRole::Primary(Some(event.0 as isize))
    }
}

#[cfg(not(windows))]
fn claim_single_instance() -> InstanceRole {
    InstanceRole::Primary(None)
}

/// 等待“第二个实例请求显示窗口”的命名事件，回到主线程调用 reveal_main。
/// 第二个进程只 SetEvent 就退出，不初始化 Tauri。
#[cfg(windows)]
fn spawn_show_watcher(event_handle: isize, app: tauri::AppHandle) {
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{ResetEvent, WaitForSingleObject, INFINITE};

    std::thread::spawn(move || {
        let event = HANDLE(event_handle as *mut _);
        loop {
            let result = unsafe { WaitForSingleObject(event, INFINITE) };
            if result != WAIT_OBJECT_0 {
                break;
            }
            let _ = unsafe { ResetEvent(event) };
            let app_for_main = app.clone();
            let _ = app.run_on_main_thread(move || reveal_main(&app_for_main, true));
        }
        let _ = unsafe { CloseHandle(event) };
    });
}

#[cfg(not(windows))]
fn spawn_show_watcher(_event_handle: isize, _app: tauri::AppHandle) {}

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

    #[cfg(windows)]
    #[test]
    fn ordinary_maximized_is_not_foreground_fullscreen() {
        use windows::Win32::UI::WindowsAndMessaging::{WS_CAPTION, WS_THICKFRAME};
        assert!(is_ordinary_maximized(true, WS_CAPTION.0 | WS_THICKFRAME.0));
        assert!(!is_ordinary_maximized(true, 0));
        assert!(!is_ordinary_maximized(false, WS_CAPTION.0));
    }

    #[test]
    fn cursor_counts_inside_the_invisible_resize_frame() {
        // 便签外框：客户区 (100,100)-(700,600)，四周还有一圈隐形的可拉伸边框
        let rect = [92, 92, 708, 608];
        assert!(point_in_rect((400, 350), rect)); // 正文里：算在窗内
        assert!(point_in_rect((95, 350), rect)); // 压在左边框上：够得着，不该缩
        assert!(point_in_rect((705, 605), rect)); // 右下角：正是拉大小的把手
        assert!(!point_in_rect((708, 608), rect)); // 右下取开区间，边界外第一个像素不算
        assert!(!point_in_rect((91, 350), rect)); // 真离开窗口：可以缩
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
        // 缺失的桌宠开关取 false、形态取 note，用户不会莫名其妙进入桌宠态。
        // close_to_tray / alarm_* 是已删功能留下的旧键，特意留着验证 serde 会忽略它们
        let legacy = r#"{"close_to_tray":false,"opacity":0.5,"yuan_shi":88,"mora":300,
            "streak":2,"weekly_settled_week":"2026-10-12","alarm_interval":600,
            "alarm_duration":15,"skin_enabled":true,"theme":"sky"}"#;
        let mut c: AppConfig = serde_json::from_str(legacy).unwrap();
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
    fn write_index_atomic_keeps_backup_and_does_not_clobber_it() {
        let dir = std::env::temp_dir().join(format!(
            "ltn-index-test-{}-{}",
            std::process::id(),
            "atomic"
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notes_index.json");
        let bak = index_bak_path(&path);

        let make = |id: &str, created: &str| NoteMeta {
            id: id.to_string(),
            title: id.to_string(),
            created_at: created.to_string(),
            updated_at: created.to_string(),
            title_edited: false,
            settled: false,
            triple_complete: false,
        };
        let first = serde_json::to_string_pretty(&vec![make("a", "2026-10-20T10:00:00")]).unwrap();
        let second = serde_json::to_string_pretty(&vec![
            make("a", "2026-10-20T10:00:00"),
            make("b", "2026-10-21T10:00:00"),
        ])
        .unwrap();

        write_index_atomic(&path, &first).unwrap();
        write_index_atomic(&path, &second).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("\"b\""));
        assert!(fs::read_to_string(&bak).unwrap().contains("\"a\""));

        // 主文件损坏时备份仍可读
        fs::write(&path, "{broken").unwrap();
        assert!(read_index_file(&path).is_none());
        assert_eq!(read_index_file(&bak).unwrap().len(), 1);

        // 从备份恢复主文件不能把好备份覆盖成损坏内容
        fs::remove_file(&path).unwrap();
        write_index_no_backup(&path, &first).unwrap();
        assert!(read_index_file(&path).is_some());
        assert!(read_index_file(&bak).is_some());

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
        assert_eq!(p.checked_count, 3);
        // 学习/工作、生活/其他 都有内容 -> 不用提醒
        assert!(p.tomorrow_filled);

        let partial = "### 每日委托\n\n#### 必做\n\n- [x] A\n\n#### 学习/工作\n\n- [ ] B\n\n#### 生活/其他\n\n- [ ] \n\n#### 随手小记\n\n- 灵感\n";
        let p = progress_of(partial);
        assert_eq!(p.modules_done, 1);
        assert!(!p.triple);
        assert!(p.has_tasks);
        assert_eq!(p.checked_count, 1);

        // 全是空白占位：一条实项都没有，has_tasks=false（宠物不该夸空气）
        let blank = "### 每日委托\n\n#### 必做\n\n- [ ] \n\n#### 学习/工作\n\n- [ ] \n\n#### 生活/其他\n\n- [ ] \n\n#### 随手小记\n\n- \n";
        let p = progress_of(blank);
        assert_eq!(p.modules_done, 0);
        assert!(!p.has_tasks);
        assert_eq!(p.checked_count, 0);
        assert!(!p.tomorrow_filled);

        // 「填了内容」不限于复选框：引用块与普通列表同样算
        let quoted = "### 每日委托\n\n#### 必做\n\n- [ ] A\n\n#### 学习/工作\n\n- [ ] \n> 明天交报告\n\n#### 生活/其他\n\n- [ ] \n";
        let p = progress_of(quoted);
        assert!(p.tomorrow_filled);
        assert_eq!(p.checked_count, 0);

        // 只有必做与随手小记写了字，两个提醒模块仍然空着
        let outside = "### 每日委托\n\n#### 必做\n\n- [x] A\n> 普通引用\n\n#### 学习/工作\n\n- [ ] \n\n#### 生活/其他\n\n- [ ] \n\n#### 随手小记\n\n> 摘抄\n";
        let p = progress_of(outside);
        assert!(!p.tomorrow_filled);
        assert_eq!(p.checked_count, 1);
    }

    #[test]
    fn section_has_content_ignores_placeholders_and_fences() {
        // 模板留下的空白占位与裸列表符都不算内容
        assert!(!section_has_content("- [ ] "));
        assert!(!section_has_content("- [ ] \n- [x] "));
        assert!(!section_has_content("-\n-\t"));
        assert!(section_has_content("- [ ] 看书 30 分钟"));
        assert!(section_has_content("> 明天交报告"));
        assert!(section_has_content("随手记一句话"));
        assert!(!section_has_content(""));
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
        // 等待解除占用 / 隐藏窗口（单实例后给旧进程 3 秒退出）
        assert!(s.contains("timeout /t 3 /nobreak"));
        // 后缀一致性：改名产物、删除、回滚必须都指向 .exe.old（防 xxx.old 残留/回滚失效）
        let old_refs = s.matches("%TGT%.old").count();
        assert!(old_refs >= 2, "删除与回滚均应引用 .exe.old，实际 {old_refs} 处");
        assert!(s.contains("%~nx1.old"), "改名产物应为 <name>.exe.old");
        assert!(!s.contains("%~n1.old"), "不得再使用无扩展名的 <name>.old");
    }

    #[test]
    fn weekly_settle_is_idempotent_and_does_not_punish_midweek_checks() {
        // 上一周已发奖，本周达标：发奖 100*(streak+1)，连续周数 +1
        let (e1, r1, s1) = weekly_settle_logic(5, 2, false, true);
        assert!(e1);
        assert_eq!(r1, 300);
        assert_eq!(s1, 3);

        // 同周再次查看：不再发奖，streak 保持不变（幂等）
        let (e2, r2, s2) = weekly_settle_logic(5, s1, true, false);
        assert!(e2);
        assert_eq!(r2, 0);
        assert_eq!(s2, 3);

        // 周中查看且未达标：不能提前清零（上一周已发奖）
        let (e3, r3, s3) = weekly_settle_logic(3, 3, false, true);
        assert!(!e3);
        assert_eq!(r3, 0);
        assert_eq!(s3, 3);

        // 进入新的一周且上一周未发奖：连击中断；本周达标从 1 开始
        let (e4, r4, s4) = weekly_settle_logic(5, 3, false, false);
        assert!(e4);
        assert_eq!(r4, 100);
        assert_eq!(s4, 1);

        // 满 7 周：仍发 700，随后连续周数归零
        let (e5, r5, s5) = weekly_settle_logic(5, 6, false, true);
        assert!(e5);
        assert_eq!(r5, 700);
        assert_eq!(s5, 0);
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
    fn fenced_code_is_not_parsed_as_commission_structure() {
        let content = "### 每日委托\n\n#### 必做\n\n- [x] A\n\n```md\n#### 学习/工作\n- [x] fake\n```\n\n#### 学习/工作\n\n- [x] B\n\n~~~\n- [x] fake2\n~~~\n\n#### 生活/其他\n\n- [x] C\n";
        let sections = parse_md_sections(content);
        assert_eq!(sections.get("必做").map(String::as_str), Some("- [x] A"));
        assert_eq!(sections.get("学习/工作").map(String::as_str), Some("- [x] B"));
        assert_eq!(sections.get("生活/其他").map(String::as_str), Some("- [x] C"));
        // 代码块里的“已勾选任务”不能算进摩拉
        assert_eq!(mora_reward(&sections), 300);
        assert_eq!(settle_reward(&sections), (50, true));
    }

    #[test]
    fn latest_daily_uses_created_date_not_updated_at() {
        let make = |id: &str, created: &str, updated: &str, settled: bool| NoteMeta {
            id: id.to_string(),
            title: id.to_string(),
            created_at: created.to_string(),
            updated_at: updated.to_string(),
            title_edited: false,
            settled,
            triple_complete: false,
        };
        let notes = vec![
            // 旧便签被编辑过，updated_at 最新，但它不该被选成“上一份”
            make("old", "2026-10-18T10:00:00", "2026-10-22T09:00:00", false),
            make("prev", "2026-10-20T10:00:00", "2026-10-20T10:00:00", false),
            make("non-daily", "2026-10-21T10:00:00", "2026-10-21T10:00:00", false),
            make("today", "2026-10-22T10:00:00", "2026-10-22T10:00:00", false),
            make("settled", "2026-10-21T10:00:00", "2026-10-21T10:00:00", true),
        ];
        let mut load = |n: &NoteMeta| {
            Some(if n.id == "non-daily" {
                "普通笔记".to_string()
            } else {
                "### 每日委托\n\n#### 必做\n- [x] A\n".to_string()
            })
        };

        // 结算：排除今天、非每日、已结算；按 created_at 选 10-20，而不是 updated_at 最新的 old
        let (idx, _) = latest_daily_before(&notes, "2026-10-22", true, &mut load).unwrap();
        assert_eq!(notes[idx].id, "prev");

        // 继承：允许已结算；仍然按 created_at，选 10-21 的 settled
        let (idx, _) = latest_daily_before(&notes, "2026-10-22", false, &mut load).unwrap();
        assert_eq!(notes[idx].id, "settled");
    }

    #[test]
    fn inherit_keeps_indented_task_content() {
        let prev = "### 每日委托\n\n#### 必做\n\n- [x] A\n  - 说明一\n  - [x] 子任务\n\n#### 学习/工作\n\n- [ ] B\n  续行 B\n    - 嵌套项\n- [x] C\n\n#### 生活/其他\n\n- [ ] \n\n#### 随手小记\n\n- 灵感\n";
        let result = inherit_from_prev(prev);
        assert!(result.contains("- [ ] A\n  - 说明一\n  - [ ] 子任务"));
        assert!(result.contains("- [ ] B\n  续行 B\n    - 嵌套项"));
        // 已完成的 C 不继承
        assert!(!result.contains("- [x] C"));
        assert!(!result.contains("灵感"));

        // 顶层完成、但嵌套子项未完成：旧行为会把子项带出来，保持
        let checked_parent = "### 每日委托\n\n#### 学习/工作\n\n- [x] 完成的主任务\n  - [ ] 还没做的子任务\n";
        let result = inherit_from_prev(checked_parent);
        assert!(result.contains("  - [ ] 还没做的子任务"));
        assert!(!result.contains("- [x] 完成的主任务"));

        // 任务块里的缩进引用不应再被“明日委托”循环重复转换
        let nested_quote = "### 每日委托\n\n#### 学习/工作\n\n- [ ] B\n  > 子引用\n";
        let result = inherit_from_prev(nested_quote);
        assert_eq!(result.matches("子引用").count(), 1);
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
    fn registry_value_data_is_quoted_path_without_args() {
        let data = autostart_reg_value_data(r"C:\Program Files\轻量便签\app.exe");
        // 不带 --startup：开机自启也照常显示窗口，那个开关已经没有读取方
        assert_eq!(data, r#""C:\Program Files\轻量便签\app.exe""#);
    }

    #[test]
    fn autostart_exe_path_strips_quotes_and_legacy_flag() {
        // 新形式：只有带引号的路径
        assert_eq!(
            autostart_exe_path(r#""C:\Program Files\轻量便签\app.exe""#),
            r"C:\Program Files\轻量便签\app.exe"
        );
        // 早期形式：末尾挂着 --startup
        assert_eq!(
            autostart_exe_path(r#""C:\Program Files\轻量便签\app.exe" --startup"#),
            r"C:\Program Files\轻量便签\app.exe"
        );
        // 更早的 VBS 时代：连引号都没有
        let legacy = r"C:\Users\liu\AppData\Local\Programs\LightweightStickyNotes\轻量便签.exe";
        assert_eq!(autostart_exe_path(legacy), legacy);
        // 迁移用的比较式：现存数据 == 规范形式时不该再写一次注册表
        let canonical = autostart_reg_value_data(legacy);
        assert_eq!(autostart_exe_path(&canonical), legacy);
        assert_eq!(autostart_reg_value_data(&autostart_exe_path(&canonical)), canonical);
    }

    #[test]
    fn value_name_is_not_legacy_name_deleted_by_cleanup() {
        assert_ne!(autostart_reg_value(), "lightweight-sticky-notes");
        assert_eq!(autostart_reg_value(), "LightweightStickyNotes");
    }

    #[test]
    fn pick_release_assets_rules() {
        // setup + portable 附件分别选中
        let release: serde_json::Value = serde_json::from_str(
            r#"{"tag_name":"v0.2.0","assets":[
                {"name":"更新说明.md","browser_download_url":"https://github.com/x/notes.md"},
                {"name":"轻量便签_0.2.0_x64-setup.exe","browser_download_url":"https://github.com/x/setup.exe"},
                {"name":"轻量便签_0.2.0_x64-portable.exe","browser_download_url":"https://github.com/x/portable.exe"}
            ]}"#,
        )
        .unwrap();
        let (setup, portable) = pick_release_assets(&release);
        assert_eq!(setup.as_deref(), Some("https://github.com/x/setup.exe"));
        assert_eq!(portable.as_deref(), Some("https://github.com/x/portable.exe"));

        // 无 portable 附件时回退到不含 setup 的 .exe
        let fallback: serde_json::Value = serde_json::from_str(
            r#"{"assets":[
                {"name":"app-setup.exe","browser_download_url":"https://github.com/x/s.exe"},
                {"name":"lightweight-sticky-notes.exe","browser_download_url":"https://github.com/x/app.exe"}
            ]}"#,
        )
        .unwrap();
        let (setup, portable) = pick_release_assets(&fallback);
        assert_eq!(setup.as_deref(), Some("https://github.com/x/s.exe"));
        assert_eq!(portable.as_deref(), Some("https://github.com/x/app.exe"));

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
    let show_event = match claim_single_instance() {
        InstanceRole::Primary(event) => event,
        InstanceRole::Secondary => return,
    };
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
                        & !tauri_plugin_window_state::StateFlags::POSITION
                        // 双击标题栏最大化是临时状态，退出时不该让插件在下次启动恢复
                        // 最大化而压过我们自己存的 note 几何
                        & !tauri_plugin_window_state::StateFlags::MAXIMIZED
                        & !tauri_plugin_window_state::StateFlags::FULLSCREEN,
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
            enable_autostart_cmd,
            disable_autostart_cmd,
            is_autostart_enabled_cmd,
            cleanup_old_autostart_cmd,
            get_opacity,
            set_opacity,
            get_theme,
            set_theme,
            get_skin,
            save_skin,
            remove_skin,
            get_ui_state,
            save_ui_state,
            get_today_progress,
            set_pet_config,
            get_idle_seconds,
            is_foreground_fullscreen,
            is_mouse_button_down,
            is_cursor_on_window,
            check_update,
            download_and_install,
        ])
        .setup(move |app| {
            #[cfg(windows)]
            if let Some(event) = show_event {
                spawn_show_watcher(event, app.handle().clone());
            }
            #[cfg(not(windows))]
            let _ = show_event;

            let app_handle = app.handle().clone();
            migrate_autostart_entry();
            cleanup_old_autostart(&app_handle);

            // 清理上次免安装版自更新留下的 .exe.old（旧进程可能尚未退出，失败则下次再清）
            if let Ok(exe) = std::env::current_exe() {
                let _ = fs::remove_file(exe.with_extension("exe.old"));
            }

            let _tray = build_tray(app)?;

            let window = app.get_webview_window("main").unwrap();

            // 开机自启也照常显示窗口：窗在 tauri.conf.json 里以 visible:false 创建，
            // 是为了让我们先把存档几何落好再露脸，避免「先跳到默认位置再挪回去」。
            let _ = window.show();

            let event_handle = app_handle.clone();
            window.on_window_event(move |event| {
                // 关闭即退出。必须显式 exit：气泡是独立窗口，主窗自己销毁后进程还活着，
                // 那时托盘「显示」已经找不到主窗了。
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    event_handle.exit(0);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
