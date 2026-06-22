# 轻量便签 (Lightweight Sticky Notes)

一个基于 **Tauri 2 + React 19 + Milkdown** 构建的 Windows 半透明 Markdown 便签应用。绿色免安装，11MB，轻量常驻。

## 功能

- **Markdown WYSIWYG 编辑** — 所见即所得编辑器，支持标题、列表、粗体、代码块、表格、引用等常用语法
- **任务列表** — `- [ ]` 复选框，点击即可标记完成
- **模板系统** — 预设每日待办、会议记录、周计划模板；可保存自定义模板
- **多便签管理** — 侧边栏列表切换、双击重命名、右键菜单（重命名 / 保存为模板 / 删除）
- **开机自启动** — 登录后自动启动并最小化到系统托盘
- **窗口置顶** — 一键将便签置顶，不被其他窗口覆盖
- **半透明 Acrylic 材质** — Windows 原生亚克力模糊效果，可调节透明度
- **位置记忆** — 关闭时自动保存窗口位置和大小，再次打开时恢复
- **系统托盘** — 关闭窗口后驻留托盘，左键点开 / 右键新建
- **暗色模式** — 浅色/深色主题切换，编辑区完整适配

## 安装

### 绿色版（推荐）

下载 `LightweightStickyNotes_Portable.exe`，放到任意目录，双击运行。

### 安装版

运行 `轻量便签_0.1.0_x64-setup.exe`，按提示安装。默认路径：

```text
%LOCALAPPDATA%\Programs\LightweightStickyNotes
```

## 系统要求

- Windows 10 1803+ 或 Windows 11（内置 WebView2）
- 64 位系统

## 技术栈

| 层级 | 技术 |
| ---- | ---- |
| 桌面框架 | Tauri 2 (Rust) |
| 前端 | React 19 + TypeScript |
| 编辑器 | Milkdown Crepe (ProseMirror) |
| 状态管理 | Zustand |
| 打包 | NSIS / 绿色免安装 |

## 开发

### 环境要求

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/) (通过 `rustup` 安装)

### 启动开发模式

```bash
npm install
npm run tauri dev
```

### 构建生产版本

```bash
npm run tauri build
```

## 数据存储

便签和模板数据存储在：

```text
%APPDATA%\com.liurr.lightweight-sticky-notes\
├── notes_index.json         # 便签索引
├── templates_index.json     # 模板索引
├── notes\
│   └── <uuid>.md            # 便签内容
└── templates\
    └── <uuid>.md            # 模板内容
```

## 项目结构

```text
notebook/
├── src/                     # React 前端
│   ├── components/          # UI 组件
│   │   ├── TitleBar.tsx     # 标题栏（拖拽、置顶、透明度）
│   │   ├── Sidebar.tsx      # 便签列表（重命名、右键菜单）
│   │   ├── NoteEditor.tsx   # Milkdown 编辑器
│   │   ├── TemplatePicker.tsx # 模板选择
│   │   └── SettingsPanel.tsx  # 设置（自启、暗色、模板管理）
│   ├── stores/              # Zustand 状态管理
│   └── types/               # TypeScript 类型
├── src-tauri/               # Rust 后端
│   └── src/
│       ├── main.rs          # 入口
│       └── lib.rs           # 命令（CRUD、模板）、托盘
└── package.json
```

## License

MIT
