use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Emitter;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
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
    let existing = read_templates_index(app);
    if existing.iter().any(|t| t.preset) {
        return; // already initialized
    }

    let presets = vec![
        (
            "每日待办",
            "# 每日待办\n\n- [ ] 任务一\n- [ ] 任务二\n- [ ] 任务三\n- [ ] 任务四\n",
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
    let mut templates = existing;
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
}

fn timestamp_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = (secs / 86400) as i64;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let mins = (time_secs % 3600) / 60;
    let secs_rem = time_secs % 60;

    let (y, m, d) = days_to_ymd(days + 719468);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        y, m, d, hours, mins, secs_rem
    )
}

fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    let era = (if days >= 0 { days } else { days - 146096 }) / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[tauri::command]
fn list_notes(app: tauri::AppHandle) -> Vec<NoteMeta> {
    read_index(&app)
}

#[tauri::command]
fn create_note(app: tauri::AppHandle, title: String) -> Result<NoteMeta, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = timestamp_now();

    let note = NoteMeta {
        id: id.clone(),
        title,
        created_at: now.clone(),
        updated_at: now,
    };

    let md_path = notes_dir(&app).join(format!("{}.md", &id));
    fs::write(&md_path, "").map_err(|e| e.to_string())?;

    let mut notes = read_index(&app);
    notes.push(note.clone());
    write_index(&app, &notes);

    Ok(note)
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

    let mut notes = read_index(&app);
    if let Some(note) = notes.iter_mut().find(|n| n.id == id) {
        note.updated_at = timestamp_now();
        if let Some(first_line) = content.lines().next() {
            let title: String = first_line
                .trim_start_matches('#')
                .trim()
                .chars()
                .take(50)
                .collect();
            if !title.is_empty() {
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
    }
    write_index(&app, &notes);
    Ok(())
}

#[tauri::command]
fn load_note(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let md_path = notes_dir(&app).join(format!("{}.md", &id));
    fs::read_to_string(&md_path).map_err(|e| e.to_string())
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
    let id = uuid::Uuid::new_v4().to_string();
    let now = timestamp_now();

    // Extract title from first heading
    let title = template_content
        .lines()
        .next()
        .map(|l| l.trim_start_matches('#').trim().chars().take(50).collect())
        .unwrap_or_else(|| "无标题便签".into());

    let note = NoteMeta {
        id: id.clone(),
        title,
        created_at: now.clone(),
        updated_at: now,
    };

    let md_path = notes_dir(&app).join(format!("{}.md", &id));
    fs::write(&md_path, &template_content).map_err(|e| e.to_string())?;

    let mut notes = read_index(&app);
    notes.push(note.clone());
    write_index(&app, &notes);

    Ok(note)
}

fn load_template_internal(app: &tauri::AppHandle, id: &str) -> Result<String, String> {
    let md_path = templates_dir(app).join(format!("{}.md", id));
    fs::read_to_string(&md_path).map_err(|e| e.to_string())
}

fn build_tray(app: &tauri::App) -> Result<tauri::tray::TrayIcon, Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let icon = app.default_window_icon().unwrap().clone();

    let show = MenuItemBuilder::with_id("show", "显示窗口").build(app)?;
    let new_note = MenuItemBuilder::with_id("new_note", "新建便签").build(app)?;
    let autostart = MenuItemBuilder::with_id("autostart", "开机自启：关").build(app)?;
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
                "autostart" => {}
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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
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
        ])
        .setup(|app| {
            #[cfg(windows)]
            {
                let window = app.get_webview_window("main").unwrap();
                if let Err(e) = window_vibrancy::apply_acrylic(&window, None) {
                    eprintln!("Failed to apply acrylic: {e}");
                }
            }

            let _tray = build_tray(app)?;

            let window = app.get_webview_window("main").unwrap();
            let w_clone = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = w_clone.hide();
                }
            });

            if std::env::args().any(|a| a == "--hidden") {}
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
