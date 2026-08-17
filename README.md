# 轻量便签 (Lightweight Sticky Notes)

<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="轻量便签 Logo" width="96" />
</p>

<p align="center">
  一个基于 <strong>Tauri 2 + React 19 + Milkdown</strong> 构建的 Windows 半透明 Markdown 便签应用。
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-2.0-24C8D8?logo=tauri&logoColor=white" alt="Tauri" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=white" alt="React" />
  <img src="https://img.shields.io/badge/TypeScript-5.8-3178C6?logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/Vite-6-646CFF?logo=vite&logoColor=white" alt="Vite" />
  <img src="https://img.shields.io/badge/License-MIT-green.svg" alt="License" />
</p>

> 🎯 绿色免安装，约 11MB，轻量常驻桌面。

---

## 目录

- [产品亮点](#产品亮点)
- [功能特性](#功能特性)
- [安装](#安装)
- [使用指南](#使用指南)
- [快捷键](#快捷键)
- [截图展示](#截图展示)
- [技术栈](#技术栈)
- [数据存储](#数据存储)
- [开发](#开发)
- [项目结构](#项目结构)
- [常见问题](#常见问题)
- [路线图](#路线图)
- [更新日志](#更新日志)
- [贡献指南](#贡献指南)
- [License](#license)

---

## 产品亮点

| 亮点 | 说明 |
| ---- | ---- |
| 🪶 极致轻量 | 绿色版仅约 11MB，无需安装即可运行，无额外运行时依赖。 |
| 📝 Markdown 原生 | 采用 Milkdown Crepe WYSIWYG 编辑器，内容即为标准 Markdown。 |
| 🎯 每日委托 | 每天自动新建一份「每日委托」，智能继承前一天未完成事项。 |
| 🪨 原石激励 | 完成委托可累积「原石」，将每日任务变成看得见的积累。 |
| 🏆 砺行修远 | 每周挑战：≥5 天三全完成委托，获得成倍原石奖励。 |
| 🎨 半透明 Acrylic | 支持 25%~100% 透明度调节，工作时可见但不遮挡。 |
| 🌙 深浅主题 | 完整的浅色 / 暗色模式适配，编辑区同样沉浸。 |

---

## 功能特性

### 📝 Markdown 编辑

所见即所得 (WYSIWYG) 编辑器，支持常用 Markdown 语法：

- 标题 `# ~ ######`、粗体 `**text**`、斜体 `*text*`
- 有序列表、无序列表、任务列表 `- [ ]` / `- [x]`
- 代码块、行内代码、引用块、分隔线
- 表格、链接、图片
- 点击任务列表方框即可标记完成 ✅

### 🎯 每日委托

每天打开应用时自动创建一份「每日委托」（若当天已有则不会重复创建），包含四个模块：

| 模块 | 说明 |
| ---- | ---- |
| 必做 | 每天必须完成的事项，新委托会**全部继承**前一天内容 |
| 学习/工作 | 学习与工作任务，新委托只继承**未完成**的项 |
| 生活/其他 | 生活杂项，新委托只继承**未完成**的项 |
| 随手小记 | 自由记录区，**不继承**、每天空白，随手记下想法与灵感 |

### 🪨 原石奖励（每日结算）

打开应用时会自动**结算最近一份尚未结算的每日委托**：

- **必做 / 学习·工作 / 生活·其他** 每一项的待办**全部勾选**即视为完成（空模块视为完成）
- 每完成一项 → **+10 原石**
- 三项全部完成 → 额外 **+20 原石**（即三项全完成共 **+50**）
- 原石余额累加，永久保留

### 🏆 砺行修远（周挑战）

标题栏「砺行修远」按钮，点击查看并结算**本周（周一至今日）**的委托完成进度：

- 本周内有 **≥5 天** 三项全部完成 → 获得 **100 × 连续达标周数** 原石
- 连续达标第 1 周 +100、第 2 周 +200 …… 第 7 周 +700 后清零
- 中断（某周未达标）→ 连续周数清零
- 弹窗会展示本周每天的完成情况

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

- **委托列表**：侧边栏按日期倒序展示每日委托，点击切换，双击标题重命名
- **右键菜单**：重命名 / 删除委托
- **系统托盘**：关闭窗口后驻留托盘，左键唤出，右键菜单（显示窗口 / 退出）
- **开机自启动**：设置中开启，登录后自动最小化到托盘
- **位置记忆**：自动保存窗口位置和大小，重启恢复
- **自动保存**：编辑后 500ms 自动保存，无需手动操作

---

## 安装

### 🟢 绿色版（推荐）

下载 `LightweightStickyNotes_Portable.exe`，放到任意目录，双击运行。

无需安装、无卸载残留。搬家或分享直接复制这一个文件即可。

### 📦 安装版

运行 `轻量便签_0.1.1_x64-setup.exe`，按提示完成安装。

- 默认路径：`%LOCALAPPDATA%\Programs\LightweightStickyNotes`
- 自动创建桌面和开始菜单快捷方式
- 在「设置 → 应用」中可正常卸载

### 系统要求

- Windows 10 1803+ 或 Windows 11
- 64 位系统
- 无需额外安装运行时（系统内置 WebView2）

---

## 使用指南

### 每日委托

1. 每天首次打开应用，自动创建当天「每日委托」（标题如 `0816`）
2. 在各模块中填入待办事项，完成打勾 `- [x]`
3. 全部内容自动保存
4. 侧边栏点击可切换历史委托；双击标题可重命名

### 获取原石

把每天委托中三个模块（必做 / 学习·工作 / 生活·其他）的任务**全部勾选**。第二天打开应用时自动结算：

- 完成 1 项 +10、2 项 +20、3 项全完成 +50 原石

### 砺行修远周挑战

1. 点击标题栏「砺行修远」
2. 查看本周（周一~今日）每天是否三全完成
3. 若本周 ≥5 天三全完成 → 自动奖励 `100 × 连续达标周数` 原石
4. 连续 7 周后清零重新开始；中断则清零

### 任务列表

在编辑器中输入 `- [ ] 任务描述` 创建一个待办项。点击方框即可在「未完成 / 已完成」之间切换：

```markdown
- [ ] 未完成的任务
- [x] 已完成的任务
```

---

## 快捷键

> 以下为默认快捷键映射；快捷键自定义在路线图中：

| 快捷键 | 功能 |
| ------ | ---- |
| `Ctrl + ,` | 打开设置 |
| `Ctrl + =` / `Ctrl + -` | 增加 / 降低窗口透明度 |
| `Ctrl + Shift + T` | 切换置顶状态 |
| `Ctrl + Shift + L` | 切换浅色 / 暗色主题 |
| `Alt + F4` | 关闭窗口 |

---

## 截图展示

> 下列截图基于 v0.1.1 版本，实际界面可能随版本迭代略有差异。

| 浅色模式编辑 | 暗色模式编辑 | 委托列表 |
| ------------ | ------------ | -------- |
| ![浅色模式](docs/screenshots/light-mode.png) | ![暗色模式](docs/screenshots/dark-mode.png) | ![委托列表](docs/screenshots/sidebar.png) |

| 设置面板 | 系统托盘菜单 | 砺行修远 |
| -------- | ------------ | -------------- |
| ![设置](docs/screenshots/settings.png) | ![托盘](docs/screenshots/tray.png) | ![砺行修远](docs/screenshots/lixing.png) |

> 提示：首次运行若未准备截图，上述占位图不会显示。可在 `docs/screenshots/` 目录放入同名 PNG 后重新查看。

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
├── config.json               # 应用配置（关闭行为、主题、透明度、原石、砺行周数）
├── notes_index.json          # 委托索引（含结算状态）
├── notes\
│   └── <uuid>.md             # 委托内容（标准 Markdown 格式）
```

备份数据只需复制整个文件夹。委托内容为标准 `.md` 文件，可用任意 Markdown 编辑器打开。

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
- 安装版：`src-tauri/target/release/bundle/nsis/轻量便签_0.1.1_x64-setup.exe`

### 开发脚本

```bash
npm run dev        # 单独启动前端 Vite 开发服务器
npm run build      # 构建前端产物
npm run preview    # 预览前端产物
npm run tauri dev  # 启动 Tauri 开发模式
```

---

## 项目结构

```text
notebook/
├── src/                          # React 前端
│   ├── assets/
│   │   └── yuan-shi.png          # 原石图标
│   ├── components/
│   │   ├── TitleBar.tsx          # 标题栏（原石余额、砺行修远、置顶、透明度、最小/关闭）
│   │   ├── Sidebar.tsx           # 委托列表（切换、重命名、右键菜单）
│   │   ├── NoteEditor.tsx        # Milkdown Crepe 编辑器封装
│   │   ├── SettingsPanel.tsx     # 设置（自启、暗色、关闭行为）
│   │   ├── FirstCloseDialog.tsx  # 首次关闭行为选择
│   │   ├── UpdateDialog.tsx      # 检查更新弹窗
│   │   └── LixingDialog.tsx      # 砺行修远结算弹窗
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
│   │   └── lib.rs                # 命令定义、委托结算、砺行修远、托盘、Acrylic
│   ├── capabilities/
│   │   └── default.json          # 权限声明
│   ├── icons/                    # 应用图标
│   ├── tauri.conf.json           # Tauri 配置
│   └── Cargo.toml
├── docs/                         # 文档与截图
├── releases/                     # 安装版发布包
├── package.json
├── vite.config.ts
└── tsconfig.json
```

---

## 常见问题

### Q1：为什么绿色版首次启动较慢？

Windows Defender 等杀毒软件会对首次运行的 exe 进行扫描，这是正常现象。后续启动会更快。

### Q2：如何备份我的委托和原石？

复制 `%APPDATA%\com.liurr.lightweight-sticky-notes\` 整个文件夹到安全位置即可。重装系统或换电脑后粘贴回原路径即可恢复（原石余额在 `config.json` 中）。

### Q3：能否在 macOS / Linux 上运行？

当前版本主要针对 Windows 打包与测试。Tauri 2 本身支持跨平台，源码具备跨平台潜力，欢迎提交 PR 扩展构建流程。

### Q4：如何彻底卸载？

- 绿色版：直接删除 exe 与数据文件夹即可。
- 安装版：在「设置 → 应用」中卸载，数据文件夹需手动删除。

### Q5：任务列表的勾选状态不同步？

请确保点击的是编辑器渲染出的任务列表复选框。若仍有问题，可尝试切换委托后重新进入。

### Q6：为什么没有结算到原石？

- 委托必须**跨天**后才会结算（当天创建的委托在第二天打开应用时结算）。
- 只有「必做 / 学习·工作 / 生活·其他」三个模块参与结算，需全部勾选才算完成。

---

## 路线图

- [x] Markdown WYSIWYG 编辑
- [x] 每日委托 + 智能继承
- [x] 原石结算奖励
- [x] 砺行修远周挑战
- [x] 置顶、透明度、暗色模式
- [x] 系统托盘与开机自启
- [x] 自动保存与窗口位置记忆
- [x] 快捷键绑定（Ctrl/,、Ctrl+=/-、Shift+T/L）
- [x] 检查更新（gitee 自动更新）
- [ ] 快捷键自定义
- [ ] 云同步 / WebDAV 备份
- [ ] 委托标签
- [ ] 原石统计图表
- [ ] 插件扩展机制

---

## 更新日志

### v0.2.0（2026-08-16）

- 新增「原石」代币体系：每日委托结算奖励，可累加
- 新增「每日委托」：每天自动创建，智能继承前一天未完成项
- 新增「砺行修远」周挑战：≥5 天三全完成奖励 100× 连续周数原石
- 移除模板机制、Markdown 导入导出、新建便签
- 便签列表改名为委托列表
- 应用图标更换

### v0.1.1（2026-07-12）

- 新增便签搜索（侧边栏按标题实时过滤）
- 新增 Markdown 导入 / 导出（单条导出、批量导出、从 .md 导入）
- 文档与版本号同步

### v0.1.0（2026-06-24）

- 初始版本发布
- 支持 Markdown WYSIWYG 编辑
- 支持多便签、模板系统、每日待办智能继承
- 支持 Acrylic 半透明窗口、置顶、透明度、暗色模式
- 支持系统托盘、开机自启、自动保存、窗口位置记忆

---

## 贡献指南

欢迎 Issue 和 PR！在提交代码前，请确保：

1. 代码遵循项目现有 TypeScript / Rust 风格
2. 通过 `npm run build` 成功构建
3. 在 PR 中说明改动目的与影响范围

---

## License

MIT © 2026 HonestLeon
