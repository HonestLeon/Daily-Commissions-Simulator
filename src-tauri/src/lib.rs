use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use chrono::Local;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub title_edited: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemplateMeta {
    pub id: String,
    pub name: String,
    pub preset: bool,
    pub created_at: String,
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

// ── Template helpers ──

fn templates_dir(app: &tauri::AppHandle) -> PathBuf {
    let path = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("templates");
    fs::create_dir_all(&path).ok();
    path
}

fn templates_index_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("templates_index.json")
}

fn read_templates_index(app: &tauri::AppHandle) -> Vec<TemplateMeta> {
    let path = templates_index_path(app);
    if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        vec![]
    }
}

fn write_templates_index(app: &tauri::AppHandle, templates: &[TemplateMeta]) {
    let path = templates_index_path(app);
    let data = serde_json::to_string_pretty(templates).unwrap_or_default();
    fs::write(&path, data).ok();
}

fn ensure_preset_templates(app: &tauri::AppHandle) {
    let preset_version: u32 = 4;
    let version_path = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("preset_version.txt");
    let stored_version: u32 = fs::read_to_string(&version_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    if stored_version >= preset_version {
        return; // up to date
    }

    // Remove old presets
    let existing = read_templates_index(app);
    for t in &existing {
        if t.preset {
            let md_path = templates_dir(app).join(format!("{}.md", &t.id));
            fs::remove_file(&md_path).ok();
        }
    }
    let mut templates: Vec<TemplateMeta> = existing.into_iter().filter(|t| !t.preset).collect();

    let presets: Vec<(&str, &str)> = vec![
        (
            "每日待办",
            "### 每日待办\n\n#### 必做\n\n- [ ] \n- [ ] \n\n#### 学习/工作\n\n- [ ] \n- [ ] \n\n#### 生活\n\n- [ ] \n- [ ] \n\n#### 其他\n\n- [ ] \n- [ ] \n",
        ),
        (
            "会议记录",
            "# 会议记录\n\n**日期：**  \n**参会人：**  \n\n## 议题\n\n- \n\n## 决议\n\n- \n\n## 待办\n\n- [ ] \n",
        ),
        (
            "周计划",
            "# 周计划\n\n## 本周目标\n\n- \n\n## 周一\n- [ ] \n## 周二\n- [ ] \n## 周三\n- [ ] \n## 周四\n- [ ] \n## 周五\n- [ ] \n",
        ),
    ];

    let now = timestamp_now();
    for (name, content) in &presets {
        let id = uuid::Uuid::new_v4().to_string();
        let meta = TemplateMeta {
            id: id.clone(),
            name: name.to_string(),
            preset: true,
            created_at: now.clone(),
        };
        let md_path = templates_dir(app).join(format!("{}.md", &id));
        fs::write(&md_path, content).ok();
        templates.push(meta);
    }
    write_templates_index(app, &templates);
    fs::write(&version_path, preset_version.to_string()).ok();
}

/// One-time migration: infer template_id and title_edited for existing notes.
fn migrate_note_template_ids(app: &tauri::AppHandle) {
    let templates = read_templates_index(app);
    if templates.is_empty() {
        return;
    }

    let daily_todo_id = daily_todo_template_id(app);
    let mut notes = read_index(app);
    let mut changed = false;

    for note in notes.iter_mut() {
        // Infer template_id for notes that don't have it yet
        if note.template_id.is_none() {
            if note.title == "每日待办" {
                note.template_id = daily_todo_id.clone();
                changed = true;
            } else {
                for template in &templates {
                    if note.title == template.name {
                        note.template_id = Some(template.id.clone());
                        changed = true;
                        break;
                    }
                }
            }
        }

        // Infer title_edited: if the current title does not match any auto-generated
        // form, the user must have renamed it manually.
        if !note.title_edited {
            if is_auto_title(app, note) {
                // Keep auto titles up to date (e.g. old "每日待办" -> "每日待办0625")
                if let Some(expected) = expected_auto_title(app, note) {
                    if note.title != expected {
                        note.title = expected;
                        changed = true;
                    }
                }
            } else {
                note.title_edited = true;
                changed = true;
            }
        }
    }

    if changed {
        write_index(app, &notes);
    }
}

/// Returns the title that would be generated automatically for this note.
fn expected_auto_title(app: &tauri::AppHandle, note: &NoteMeta) -> Option<String> {
    let daily_todo_id = daily_todo_template_id(app);
    let is_daily_todo = daily_todo_id.as_ref() == note.template_id.as_ref()
        || note.title == "每日待办";
    if is_daily_todo {
        return Some(format_daily_todo_title(&note.updated_at));
    }

    if let Some(ref tid) = note.template_id {
        let templates = read_templates_index(app);
        if let Some(t) = templates.iter().find(|t| t.id == *tid) {
            return Some(t.name.clone());
        }
    }

    Some("无标题便签".to_string())
}

/// Checks whether the current title is one of the accepted auto-generated forms.
fn is_auto_title(app: &tauri::AppHandle, note: &NoteMeta) -> bool {
    let daily_todo_id = daily_todo_template_id(app);
    let is_daily_todo = daily_todo_id.as_ref() == note.template_id.as_ref()
        || note.title == "每日待办";
    if is_daily_todo {
        let expected = format_daily_todo_title(&note.updated_at);
        return note.title == expected || note.title == "每日待办";
    }

    if let Some(ref tid) = note.template_id {
        let templates = read_templates_index(app);
        if let Some(t) = templates.iter().find(|t| t.id == *tid) {
            return note.title == t.name;
        }
    }

    note.title == "无标题便签"
}

fn timestamp_now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn daily_todo_template_id(app: &tauri::AppHandle) -> Option<String> {
    read_templates_index(app)
        .iter()
        .find(|t| t.name == "每日待办")
        .map(|t| t.id.clone())
}

/// Format a daily todo title with the date suffix, e.g. "每日待办0625".
/// `iso` is expected to be in the format returned by `timestamp_now()`.
fn format_daily_todo_title(iso: &str) -> String {
    if iso.len() >= 10 {
        let month = &iso[5..7];
        let day = &iso[8..10];
        format!("每日待办{}{}", month, day)
    } else {
        "每日待办".to_string()
    }
}

fn sanitize_filename(title: &str) -> String {
    let sanitized: String = title
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();
    let sanitized = sanitized.trim().replace(' ', "_");
    if sanitized.is_empty() {
        "note".to_string()
    } else {
        sanitized
    }
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
        template_id: None,
        title_edited: false,
    };
    let md_path = notes_dir(app).join(format!("{}.md", &id));
    fs::write(&md_path, content).map_err(|e| e.to_string())?;
    let mut notes = read_index(app);
    notes.push(note.clone());
    write_index(app, &notes);
    Ok(note)
}

#[tauri::command]
fn list_notes(app: tauri::AppHandle) -> Vec<NoteMeta> {
    read_index(&app)
}

#[tauri::command]
fn create_note(app: tauri::AppHandle, title: String) -> Result<NoteMeta, String> {
    create_note_with_content(&app, title, String::new())
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
    fs::write(&md_path, &content).map_err(|e| e.to_string())?;

    let daily_todo_id = daily_todo_template_id(&app);
    let mut notes = read_index(&app);
    if let Some(note) = notes.iter_mut().find(|n| n.id == id) {
        note.updated_at = timestamp_now();

        // Only auto-update title if the user has not manually renamed the note
        if !note.title_edited {
            let base_title = content.lines().next().and_then(|first_line| {
                let trimmed = first_line.trim();
                if trimmed.starts_with('#') {
                    let title: String = trimmed
                        .trim_start_matches('#')
                        .trim()
                        .chars()
                        .take(50)
                        .collect();
                    if !title.is_empty() {
                        return Some(title);
                    }
                }
                None
            });

            // Daily todo notes always get a date suffix title, e.g. "每日待办0625"
            let is_daily_todo = daily_todo_id.as_ref() == note.template_id.as_ref()
                || base_title.as_deref() == Some("每日待办");
            if is_daily_todo {
                note.title = format_daily_todo_title(&note.updated_at);
            } else if let Some(title) = base_title {
                note.title = title;
            }
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

// ── Markdown import / export commands ──

#[tauri::command]
async fn export_note(app: tauri::AppHandle, id: String) -> Result<Option<String>, String> {
    let content = load_note_internal(&app, &id)?;
    let notes = read_index(&app);
    let note = notes
        .iter()
        .find(|n| n.id == id)
        .ok_or_else(|| "便签不存在".to_string())?;

    let default_name = format!("{}.md", sanitize_filename(&note.title));
    let path = app
        .dialog()
        .file()
        .add_filter("Markdown", &["md"])
        .set_file_name(&default_name)
        .blocking_save_file();

    if let Some(p) = path {
        let path = p.into_path().map_err(|e| e.to_string())?;
        fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(Some(path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn export_all_notes(app: tauri::AppHandle) -> Result<Option<(String, usize)>, String> {
    let notes = read_index(&app);
    if notes.is_empty() {
        return Ok(None);
    }

    let folder = app
        .dialog()
        .file()
        .set_title("导出所有便签到文件夹")
        .blocking_pick_folder();

    if let Some(dir) = folder {
        let dir = dir.into_path().map_err(|e| e.to_string())?;
        let mut count = 0;
        for note in &notes {
            let content = load_note_internal(&app, &note.id).unwrap_or_default();
            let short_id = &note.id[..note.id.len().min(8)];
            let filename = format!("{}_{}.md", sanitize_filename(&note.title), short_id);
            let path = dir.join(&filename);
            if fs::write(&path, content).is_ok() {
                count += 1;
            }
        }
        Ok(Some((dir.to_string_lossy().to_string(), count)))
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn import_notes(app: tauri::AppHandle) -> Result<Option<usize>, String> {
    let files = app
        .dialog()
        .file()
        .add_filter("Markdown", &["md"])
        .set_title("导入 Markdown 文件")
        .blocking_pick_files();

    if let Some(paths) = files {
        let mut count = 0;
        for p in paths {
            let p = p.into_path().map_err(|e| e.to_string())?;
            let content = fs::read_to_string(&p)
                .map_err(|e| format!("读取 {} 失败: {}", p.display(), e))?;
            let file_stem = p
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "导入便签".to_string());

            let title = content
                .lines()
                .find(|l| !l.trim().is_empty())
                .and_then(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with('#') {
                        Some(
                            trimmed
                                .trim_start_matches('#')
                                .trim()
                                .chars()
                                .take(100)
                                .collect(),
                        )
                    } else {
                        None
                    }
                })
                .unwrap_or(file_stem);

            create_note_with_content(&app, title, content)?;
            count += 1;
        }
        Ok(Some(count))
    } else {
        Ok(None)
    }
}

// ── Template commands ──

#[tauri::command]
fn list_templates(app: tauri::AppHandle) -> Vec<TemplateMeta> {
    ensure_preset_templates(&app);
    read_templates_index(&app)
}

#[tauri::command]
fn load_template(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let md_path = templates_dir(&app).join(format!("{}.md", &id));
    fs::read_to_string(&md_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_template(app: tauri::AppHandle, name: String, content: String) -> Result<TemplateMeta, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = timestamp_now();
    let meta = TemplateMeta {
        id: id.clone(),
        name,
        preset: false,
        created_at: now,
    };
    let md_path = templates_dir(&app).join(format!("{}.md", &id));
    fs::write(&md_path, &content).map_err(|e| e.to_string())?;
    let mut templates = read_templates_index(&app);
    templates.push(meta.clone());
    write_templates_index(&app, &templates);
    Ok(meta)
}

#[tauri::command]
fn delete_template(app: tauri::AppHandle, id: String) -> Result<(), String> {
    // Don't allow deleting preset templates
    let templates = read_templates_index(&app);
    if let Some(t) = templates.iter().find(|t| t.id == id) {
        if t.preset {
            return Err("不能删除预设模板".into());
        }
    }
    let md_path = templates_dir(&app).join(format!("{}.md", &id));
    fs::remove_file(&md_path).ok();
    let mut templates = read_templates_index(&app);
    templates.retain(|t| t.id != id);
    write_templates_index(&app, &templates);
    Ok(())
}

#[tauri::command]
fn create_note_from_template(
    app: tauri::AppHandle,
    template_id: String,
) -> Result<NoteMeta, String> {
    let template_content = load_template_internal(&app, &template_id)?;
    let templates_index = read_templates_index(&app);
    let template_name = templates_index
        .iter()
        .find(|t| t.id == template_id)
        .map(|t| t.name.clone())
        .unwrap_or_default();

    let id = uuid::Uuid::new_v4().to_string();
    let now = timestamp_now();

    let mut title = template_content
        .lines()
        .next()
        .map(|l| l.trim_start_matches('#').trim().chars().take(50).collect())
        .unwrap_or_else(|| "无标题便签".into());

    // Daily todo titles automatically include the date suffix, e.g. "每日待办0625"
    if template_name == "每日待办" {
        title = format_daily_todo_title(&now);
    }

    // Smart inheritance for daily todo
    let final_content = if template_name == "每日待办" {
        inherit_daily_todo(&app, &template_content)
    } else {
        template_content.clone()
    };

    let note = NoteMeta {
        id: id.clone(),
        title,
        created_at: now.clone(),
        updated_at: now,
        template_id: Some(template_id),
        title_edited: false,
    };

    let md_path = notes_dir(&app).join(format!("{}.md", &id));
    fs::write(&md_path, &final_content).map_err(|e| e.to_string())?;

    let mut notes = read_index(&app);
    notes.push(note.clone());
    write_index(&app, &notes);

    Ok(note)
}

/// Reset a checkbox line to unchecked state, preserving the marker style (- or *)
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

/// Find the most recent daily todo note and inherit unchecked items
fn inherit_daily_todo(app: &tauri::AppHandle, template: &str) -> String {
    let notes = read_index(app);
    let mut sorted: Vec<_> = notes.iter().collect();
    sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let prev_content = sorted.iter().find_map(|n| {
        let content = fs::read_to_string(notes_dir(app).join(format!("{}.md", &n.id))).ok()?;
        if content.contains("### 每日待办") {
            Some(content)
        } else {
            None
        }
    });

    let Some(prev) = prev_content else {
        eprintln!("[inherit] No previous daily todo found");
        return template.to_string();
    };
    eprintln!("[inherit] Found previous daily todo");

    let prev_sections = parse_md_sections(&prev);
    let template_sections = parse_md_sections(template);

    eprintln!("[inherit] prev sections: {:?}", prev_sections.keys().collect::<Vec<_>>());
    eprintln!("[inherit] tmpl sections: {:?}", template_sections.keys().collect::<Vec<_>>());

    // Build result from scratch: template heading lines + inherited items per section
    let mut result = String::new();
    let mut in_section: Option<&str> = None;

    for line in template.lines() {
        if line.starts_with("#### ") {
            // Starting a new section
            in_section = Some(line.trim_start_matches("#### ").trim());
            result.push_str(line);
            result.push('\n');
            result.push('\n');

            // Insert inherited items for this section
            if let Some(sec_name) = in_section {
                eprintln!("[inherit] Looking for section: {}", sec_name);
                let mut inserted = false;
                if let Some(prev_sec) = prev_sections.get(sec_name) {
                    eprintln!("[inherit] {} content: {:?}", sec_name, prev_sec);
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

                    eprintln!("[inherit] {} filtered items: {:?}", sec_name, raw_items);
                    if !raw_items.is_empty() {
                        for item in &raw_items {
                            // For "必做", reset checkboxes to unchecked
                            let line = if sec_name == "必做" {
                                reset_checkbox(item)
                            } else {
                                item.to_string()
                            };
                            result.push_str(&line);
                            result.push('\n');
                        }
                        eprintln!("[inherit] {} — inherited {} items", sec_name, raw_items.len());
                        inserted = true;
                    }
                }
                if !inserted {
                    // No previous items: blank checkbox
                    result.push_str("- [ ] \n");
                }
                continue; // skip template's placeholder lines for this section
            }
        }

        // Skip template's placeholder items (blank checkboxes without text)
        let trimmed = line.trim_start();
        if trimmed == "- [ ]" || trimmed == "- [ ] " || trimmed == "* [ ]" || trimmed == "* [ ] " || trimmed == "- [ ] 待办事项" || trimmed == "* [ ] 待办事项" {
            continue;
        }

        // Write non-section lines
        if in_section.is_none() || !line.trim_start().starts_with("- [") {
            result.push_str(line);
            result.push('\n');
        }
    }

    eprintln!("[inherit] Result:\n{}", result);
    result
}

/// Parse markdown into sections keyed by #### heading name
fn parse_md_sections(content: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut current_section = String::new();
    let mut current_name = String::new();

    for line in content.lines() {
        if line.starts_with("#### ") {
            // Save previous section
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
    // Save last section
    if !current_name.is_empty() {
        map.insert(current_name, current_section.trim().to_string());
    }
    map
}

fn load_template_internal(app: &tauri::AppHandle, id: &str) -> Result<String, String> {
    let md_path = templates_dir(app).join(format!("{}.md", id));
    fs::read_to_string(&md_path).map_err(|e| e.to_string())
}

// ── App config ──

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppConfig {
    close_to_tray: bool,
    dark_mode: bool,
    opacity: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            dark_mode: false,
            opacity: 0.92,
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

/// URL-encode a path segment so non-ASCII (e.g. 轻量便签) survives in a URL.
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

/// Semver-ish comparison: returns true if `a > b`.
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

    // Download the installer.
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|e| format!("下载失败：{}", e))?;
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&installer).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| format!("保存安装包失败：{}", e))?;

    // Silent install (NSIS /S). Installs to the default location.
    let status = std::process::Command::new(&installer)
        .arg("/S")
        .status()
        .map_err(|e| format!("启动安装器失败：{}", e))?;
    if !status.success() {
        return Err("安装未成功，请前往 gitee 手动下载安装".to_string());
    }

    // Try to relaunch the newly installed version, then exit the current one.
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
    // NOTE: must NOT be "lightweight-sticky-notes" — that legacy name is
    // deleted by cleanup_old_autostart(), which would remove our own entry.
    "LightweightStickyNotes"
}

/// Registry data format: `"C:\path\app.exe" --startup`
/// Quotes protect paths with spaces; --startup triggers silent tray launch.
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
        // Debug builds spawn a console window and depend on the Vite dev server;
        // registering one would cause the white window + terminal at boot.
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
            key.set_value(
                autostart_reg_value(),
                &autostart_reg_value_data(exe_path),
            )
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

#[tauri::command]
fn get_autostart() -> bool {
    is_autostart_enabled_internal()
}

#[tauri::command]
fn toggle_autostart(_app: tauri::AppHandle) -> Result<bool, String> {
    toggle_autostart_internal()
}

/// One-time migration: move the autostart entry from the legacy value name
/// ("lightweight-sticky-notes", no quotes / no --startup flag) to the current
/// one. Must run BEFORE cleanup_old_autostart, which deletes the legacy name.
fn migrate_autostart_entry() {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) =
            hkcu.open_subkey_with_flags(autostart_reg_key(), KEY_QUERY_VALUE | KEY_SET_VALUE)
        {
            if let Ok(old) = key.get_value::<String, _>("lightweight-sticky-notes") {
                // Old value is a bare exe path; re-register properly and drop the legacy name.
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

/// Clean up old VBS and registry entries from previous autostart approaches
fn cleanup_old_autostart(_app: &tauri::AppHandle) {
    // Remove old VBS file from Startup folder
    if let Ok(appdata) = std::env::var("APPDATA") {
        let vbs_path = PathBuf::from(appdata)
            .join(r"Microsoft\Windows\Start Menu\Programs\Startup")
            .join("lightweight-sticky-notes.vbs");
        if vbs_path.exists() {
            let _ = fs::remove_file(&vbs_path);
        }
    }

    // Remove old registry entry from tauri-plugin-autostart (different value name format)
    #[cfg(windows)]
    {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey_with_flags(autostart_reg_key(), winreg::enums::KEY_SET_VALUE) {
            // Try deleting old tauri-plugin-autostart entries
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
    let new_note = MenuItemBuilder::with_id("new_note", "新建便签").build(app)?;
    let autostart = MenuItemBuilder::with_id("autostart", autostart_label).build(app)?;
    let autostart_clone = autostart.clone();
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&show, &new_note, &autostart, &separator, &quit])
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
                "new_note" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                        let _ = w.emit("tray-new-note", ());
                    }
                }
                "autostart" => {
                    // On failure (e.g. debug build), keep the label in sync with reality.
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
    fn registry_value_data_has_quotes_and_startup_flag() {
        let data = autostart_reg_value_data(r"C:\Program Files\轻量便签\app.exe");
        assert_eq!(data, r#""C:\Program Files\轻量便签\app.exe" --startup"#);
    }

    #[test]
    fn value_name_is_not_legacy_name_deleted_by_cleanup() {
        assert_ne!(autostart_reg_value(), "lightweight-sticky-notes");
        assert_eq!(autostart_reg_value(), "LightweightStickyNotes");
    }

    /// Simulates the migration parsing: legacy bare path -> quoted + flag.
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
    fn startup_flag_detection() {
        let args = vec!["app.exe".to_string(), "--startup".to_string()];
        assert!(args.iter().any(|a| a == "--startup"));
        let args2 = vec!["app.exe".to_string()];
        assert!(!args2.iter().any(|a| a == "--startup"));
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
                // Don't let the plugin restore visibility: the window starts hidden
                // and is shown manually (unless launched with --startup for silent boot).
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_notes,
            create_note,
            delete_note,
            rename_note,
            save_note,
            load_note,
            list_templates,
            load_template,
            save_template,
            delete_template,
            create_note_from_template,
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
            export_note,
            export_all_notes,
            import_notes,
            check_update,
            download_and_install,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            // Migration must run before cleanup, which deletes the legacy value name.
            migrate_autostart_entry();
            cleanup_old_autostart(&app_handle);
            ensure_preset_templates(&app_handle);
            migrate_note_template_ids(&app_handle);

            let _tray = build_tray(app)?;

            let window = app.get_webview_window("main").unwrap();
            let w_clone = window.clone();

            // The window is created hidden (tauri.conf.json: visible=false) to avoid a
            // white flash. Show it on normal launches; with --startup (autostart entry)
            // stay hidden in the tray. Position/size are still restored by the plugin.
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
                    // else: allow close → app exits
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
