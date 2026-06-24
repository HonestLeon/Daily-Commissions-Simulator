# 轻量便签 (Lightweight Sticky Notes)

一个基于 **Tauri 2 + React 19 + Milkdown** 构建的 Windows 半透明 Markdown 便签应用。

🎯 绿色免安装，11MB，轻量常驻。

## 功能特性

### 📝 Markdown 编辑

所见即所得 (WYSIWYG) 编辑器，支持常用 Markdown 语法：

- 标题 `# ~ ######`、粗体 `**text**`、斜体 `*text*`
- 有序列表、无序列表、任务列表 `- [ ]` / `- [x]`
- 代码块、行内代码、引用块、分隔线
- 表格、链接、图片
- 点击任务列表方框即可标记完成 ✅

### 📋 模板系统

**预设模板**（自动创建，不可删除）：

| 模板 | 说明 |
| ---- | ---- |
| 每日待办 | 分「必做」「学习/工作」「生活」「其他」四个模块，智能继承前一天未完成项 |
| 会议记录 | 含日期、参会人、议题、决议、待办模块 |
| 周计划 | 按周一至周五分日管理任务 |

**自定义模板**：右键任意便签 →「保存为模板」，即可将当前内容存为模板。

**智能继承**（每日待办专属）：新建每日待办时，自动从上一个每日待办中：
- **必做**模块 → 全部内容按原样复制
- **学习/工作 / 生活 / 其他**模块 → 仅未打勾 `- [ ]` 的待办带入

### 🪟 窗口控制

| 按钮 | 功能 |
| ---- | ---- |
| 📌 | 一键置顶，窗口始终位于最上层 |
| 🎚️ | 透明度滑块（25%~100%），失焦状态下可见但不遮挡 |
| ⚙ | 打开设置面板 |
| ─ | 最小化到任务栏 |
| ✕ | 关闭（首次弹窗询问：托盘 / 退出，可记住选择） |

### 🌓 主题

- **浅色模式**：白色 Acrylic 亚克力半透明材质
- **暗色模式**：深色背景 + 完整编辑区适配（黑底白字）
- 在设置中一键切换

### 🔧 其他

- **多便签管理**：侧边栏列表，点击切换，双击标题重命名
- **右键菜单**：重命名 / 保存为模板 / 删除
- **系统托盘**：关闭窗口后驻留托盘，左键唤出，右键菜单（新建 / 退出）
- **开机自启动**：设置中开启，登录后自动最小化到托盘
- **位置记忆**：自动保存窗口位置和大小，重启恢复
- **自动保存**：编辑后 500ms 自动保存，无需手动操作

---

## 安装

### 🟢 绿色版（推荐）

下载 `LightweightStickyNotes_Portable.exe`，放到任意目录，双击运行。

无需安装、无卸载残留。搬家或分享直接复制这一个文件即可。

### 📦 安装版

运行 `轻量便签_0.1.0_x64-setup.exe`，按提示完成安装。

- 默认路径：`%LOCALAPPDATA%\Programs\LightweightStickyNotes`
- 自动创建桌面和开始菜单快捷方式
- 在「设置 → 应用」中可正常卸载

### 系统要求

- Windows 10 1803+ 或 Windows 11
- 64 位系统
- 无需额外安装运行时（系统内置 WebView2）

---

## 使用指南

### 基本操作

1. 启动后点击「＋ 新建便签」→ 选择模板或空白
2. 在编辑区输入 Markdown 内容
3. 所有内容自动保存，无需手动操作
4. 点击侧边栏可切换便签；双击可重命名标题
5. 点击 ─ 最小化到任务栏，点击 ✕ 关闭到托盘或退出

### 任务列表

在编辑器中输入 `- [ ] 任务描述` 创建一个待办项。点击方框即可在「未完成 / 已完成」之间切换：

```markdown
- [ ] 未完成的任务
- [x] 已完成的任务
```

### 每日待办智能继承

1. 使用「每日待办」模板创建第一条便签
2. 在各模块中填入待办事项，完成打勾
3. 第二天再次使用「每日待办」模板创建新便签
4. 自动带入：必做全部内容 + 其他模块中未完成项

---

## 技术栈

| 层级 | 技术 |
| ---- | ---- |
| 桌面框架 | Tauri 2 (Rust) |
| 前端 | React 19 + TypeScript |
| 构建工具 | Vite 6 |
| 编辑器 | Milkdown Crepe (ProseMirror) |
| 状态管理 | Zustand |
| 样式 | 纯 CSS（无框架依赖） |
| 打包 | NSIS / 绿色免安装 |

---

## 数据存储

所有数据存储在本地，不上传任何信息：

```text
%APPDATA%\com.liurr.lightweight-sticky-notes\
├── config.json               # 应用配置（关闭行为等）
├── notes_index.json          # 便签索引
├── templates_index.json      # 模板索引
├── preset_version.txt        # 预设模板版本号
├── notes\
│   └── <uuid>.md             # 便签内容（标准 Markdown 格式）
└── templates\
    └── <uuid>.md             # 模板内容
```

备份数据只需复制整个文件夹。便签内容为标准 `.md` 文件，可用任意 Markdown 编辑器打开。

---

## 开发

### 环境准备

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/)（通过 `rustup` 安装）
- Windows 系统需要 MSVC 构建工具（`winget install Microsoft.VisualStudio.2022.BuildTools`）

### 克隆与运行

```bash
git clone https://gitee.com/Honest_Leon/lightweight-sticky-notes.git
cd lightweight-sticky-notes
npm install
npm run tauri dev
```

### 构建

```bash
npm run tauri build
```

产物：
- 绿色版：`src-tauri/target/release/lightweight-sticky-notes.exe`
- 安装版：`src-tauri/target/release/bundle/nsis/轻量便签_0.1.0_x64-setup.exe`

---

## 项目结构

```text
notebook/
├── src/                          # React 前端
│   ├── components/
│   │   ├── TitleBar.tsx          # 标题栏（拖拽区、置顶、透明度、最小/关闭）
│   │   ├── Sidebar.tsx           # 便签列表（切换、重命名、右键菜单）
│   │   ├── NoteEditor.tsx        # Milkdown Crepe 编辑器封装
│   │   ├── TemplatePicker.tsx    # 模板选择对话框（分组折叠）
│   │   ├── SettingsPanel.tsx     # 设置（自启、暗色、关闭行为、模板管理）
│   │   └── FirstCloseDialog.tsx  # 首次关闭行为选择
│   ├── stores/
│   │   └── noteStore.ts          # Zustand 全局状态
│   ├── types/
│   │   └── index.ts              # TypeScript 类型定义
│   ├── App.tsx                   # 根布局
│   ├── App.css                   # 全局样式（含暗色模式）
│   └── main.tsx                  # 入口
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # 进程入口
│   │   └── lib.rs                # 命令定义、模板管理、托盘、Acrylic
│   ├── capabilities/
│   │   └── default.json          # 权限声明
│   ├── icons/                    # 应用图标
│   ├── tauri.conf.json           # Tauri 配置
│   └── Cargo.toml
├── package.json
├── vite.config.ts
└── tsconfig.json
```

## License

MIT
