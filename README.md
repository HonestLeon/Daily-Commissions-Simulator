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
> 🏆 本作品参与「TRAE AI 创造力大赛」学习工作赛道。

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
- [TRAE 实践过程](#TRAE-实践过程)
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
| 📝 Markdown 原生 | 采用 Milkdown Crepe WYSIWYG 编辑器，便签内容即为标准 Markdown。 |
| 🧠 智能继承 | 「每日待办」模板自动继承前一天未完成事项，减少重复录入。 |
| 🎨 半透明 Acrylic | 支持 25%~100% 透明度调节，工作时可见但不遮挡。 |
| 🌙 深浅主题 | 完整的浅色 / 暗色模式适配，编辑区同样沉浸。 |
| 🧩 模板系统 | 内置 3 套预设模板，支持自定义模板沉淀个人工作流。 |

---

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

### 自定义模板

1. 编辑任意便签到理想格式
2. 在侧边栏右键该便签 →「保存为模板」
3. 新建便签时即可在「我的模板」分组中看到

---

## 快捷键

> 快捷键可在后续版本中自定义，当前默认映射如下：

| 快捷键 | 功能 |
| ------ | ---- |
| `Ctrl + N` | 新建便签 |
| `Ctrl + D` | 删除当前便签 |
| `Ctrl + ,` | 打开设置 |
| `Ctrl + =` / `Ctrl + -` | 增加 / 降低窗口透明度 |
| `Ctrl + Shift + T` | 切换置顶状态 |
| `Ctrl + Shift + L` | 切换浅色 / 暗色主题 |
| `Alt + F4` | 关闭窗口 |

---

## 截图展示

> 下列截图基于 v0.1.0 版本，实际界面可能随版本迭代略有差异。

| 浅色模式编辑 | 暗色模式编辑 | 模板选择 |
| ------------ | ------------ | -------- |
| ![浅色模式](docs/screenshots/light-mode.png) | ![暗色模式](docs/screenshots/dark-mode.png) | ![模板选择](docs/screenshots/template-picker.png) |

| 设置面板 | 系统托盘菜单 | 每日待办继承 |
| -------- | ------------ | -------------- |
| ![设置](docs/screenshots/settings.png) | ![托盘](docs/screenshots/tray.png) | ![继承](docs/screenshots/inherit.png) |

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
├── config.json               # 应用配置（关闭行为、主题、透明度等）
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

## TRAE 实践过程

本作品的核心架构与功能在本地 IDE 中完成，UI 精修、问题修复、文档整理及参赛材料等关键环节使用 **TRAE Work / TRAE IDE** 协作完成。以下记录关键实践节点、Session ID 以及用 TRAE 解决的核心问题。

### 创作方式

- **本地完成核心架构与功能**：项目骨架、模板系统、智能继承逻辑、状态管理等核心代码先在本地 IDE 中完成。
- **TRAE 辅助精修与文档**：进入 UI 打磨、问题修复、文档整理和参赛材料准备阶段后，使用 TRAE Work 进行高效协作。
- **规则先拆解**：对复杂逻辑（如每日待办智能继承）先与 TRAE 一起梳理边界规则，再转化为代码。
- **问题共定位**：遇到 UI/UX 细节异常（如透明度失效、暗色模式不一致）时，由 TRAE 分析 DOM 与样式冲突并给出修复方案。
- **物料一体化**：README、在线 Demo、参赛文案由 TRAE 协助生成与润色，保持风格统一。

### 关键创作节点

> 大赛要求提供**不少于 3 个 TRAE Session ID** 作为 TRAE 创作证明。Session ID 仅对应**在 TRAE Work / TRAE IDE 中完成的创作环节**，本地独立开发的部分无需强行对应。

| 阶段 | 创作工具 | Session ID | 时间 | 核心任务 | 关键产出 |
| ---- | -------- | ---------- | ---- | -------- | -------- |
| 项目初始化与架构设计 | 本地 IDE | — | 2026-06-24 | 确定 Tauri 2 + React 19 + Milkdown 技术栈，初始化项目，设计状态管理结构 | 项目骨架、`noteStore.ts`、类型定义 |
| 核心功能实现 | 本地 IDE | — | 2026-06-24 | 实现模板系统、每日待办智能继承、Zustand 状态持久化、自动保存 | `TemplatePicker.tsx`、智能继承逻辑 |
| UI 打磨与透明效果调试 | TRAE Work | `6a3bed9546c87b1659640b4f` | 2026-06-25 | 修复 Milkdown 编辑器背景层覆盖导致透明度滑块失效的问题，完善暗色模式适配 | `src/App.css` 透明背景修复提交 |
| 文档完善与参赛准备 | TRAE Work | `6a3bed9546c87b1659640b4f` | 2026-06-25 | 扩展 README 为完整项目文档，生成 HTML Demo，整理 TRAE 实践过程 | 新版 README、`demo/index.html` |

#### 当前 Session ID 数量

目前已提供 **1 个真实 Session ID**（同一 Session 覆盖了 UI 修复与文档准备两个阶段）。

为满足大赛「不少于 3 个 Session ID」的要求，建议补齐以下任一方式：

1. **拆分已有 TRAE 工作**：如果 `6a3bed9546c87b1659640b4f` 这个长 Session 中明显分为多个独立主题（如透明度修复、侧边栏折叠修复、README 扩展、Demo 生成），可拆分为 2～3 个 Session ID 分别填写。
2. **补充新的 TRAE 创作环节**：任选 1～2 个当前尚未使用 TRAE 的功能/材料，在 TRAE 中完成并记录 Session ID，例如：
   - 在 TRAE 中生成 3 张产品截图或海报
   - 在 TRAE 中编写/润色参赛报名帖
   - 在 TRAE 中实现一个剩余路线图功能（如标签搜索、导出 PDF）
   - 在 TRAE 中制作抖音演示视频脚本

参赛提交前，请确保最终表格中至少有 **3 个不同的 TRAE Session ID**。

### 典型 AI 协作示例

#### 示例 1：侧边栏分组折叠失效修复

问题表现：点击侧边栏分组标题时，包含当前选中便签的分组无法折叠，▶/▼ 指示符无响应。

TRAE 定位原因：组件中存在「自动展开包含当前选中便签的分组」逻辑，该逻辑会在每次点击后强制展开，导致用户折叠操作被覆盖。

修复方案：移除自动展开逻辑，改由用户主动控制分组折叠状态，所有分组均可正常展开/收起。

#### 示例 2：编辑器透明度失效修复

问题表现：拖动标题栏透明度滑块后，只有窗口边框变透明，编辑区仍保持不透明白底。

TRAE 定位原因：Milkdown 编辑器内部的 `.milkdown`、`.editor`、`.ProseMirror` 等元素设置了固定白色背景，覆盖了下层 `editor-area` 的透明度。

修复方案：在 `src/App.css` 中将上述元素的背景设为透明，并让暗色模式使用基于 CSS 变量 `--app-bg-opacity` 的动态透明度，使滑块对所有区域生效。

#### 示例 3：单文件 HTML Demo 生成

提示词大意：

> 为我的轻量便签作品生成一个可在线体验的 HTML Demo，重点展示「每日待办智能继承」功能，要能部署到 GitHub Pages。

TRAE 根据项目实际 UI 和逻辑，生成了独立的 `demo/index.html`：预置昨日待办数据、提供模板选择弹窗、点击「每日待办」后自动继承未完成任务并展示提示动画，同时支持透明度与主题切换。

#### 示例 4：智能继承规则拆解（设计参考）

在本地实现「每日待办智能继承」前，先向 TRAE 咨询规则设计：

> 帮我设计一个「每日待办」模板的继承逻辑：「必做」模块整模块复制（含已完成项），其他模块只复制未完成的 `- [ ]` 任务列表项。

TRAE 给出的拆解思路为后续实现提供了清晰的模块级过滤方案，避免了简单全文复制导致旧任务反复带入的问题。

### 用 TRAE 解决的关键问题

1. **智能继承逻辑**：将「前一天未完成待办自动带入新便签」的产品需求，落地为模块级 Markdown 解析与任务过滤算法。
2. **Rust 与前端状态同步**：通过 Tauri Command 实现窗口置顶、透明度、主题等设置的持久化与跨进程通信。
3. **Milkdown 与 React 集成**：封装 Crepe 编辑器组件，处理聚焦、内容同步、任务列表点击事件与自定义样式覆盖。
4. **Acrylic 透明窗口体验**：协调 Tauri 窗口配置、CSS 变量与 Milkdown 内部样式，使透明背景在不同主题下均保持可读。

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
├── docs/                         # 文档与截图
├── package.json
├── vite.config.ts
└── tsconfig.json
```

---

## 常见问题

### Q1：为什么绿色版首次启动较慢？

Windows Defender 等杀毒软件会对首次运行的 exe 进行扫描，这是正常现象。后续启动会更快。

### Q2：如何备份我的便签？

复制 `%APPDATA%\com.liurr.lightweight-sticky-notes\` 整个文件夹到安全位置即可。重装系统或换电脑后粘贴回原路径即可恢复。

### Q3：能否在 macOS / Linux 上运行？

当前版本主要针对 Windows 打包与测试。Tauri 2 本身支持跨平台，源码具备跨平台潜力，欢迎提交 PR 扩展构建流程。

### Q4：如何彻底卸载？

- 绿色版：直接删除 exe 与数据文件夹即可。
- 安装版：在「设置 → 应用」中卸载，数据文件夹需手动删除。

### Q5：任务列表的勾选状态不同步？

请确保点击的是编辑器渲染出的任务列表复选框。若仍有问题，可尝试切换便签后重新进入。

---

## 路线图

- [x] Markdown WYSIWYG 编辑
- [x] 多便签管理与侧边栏
- [x] 预设模板 + 自定义模板
- [x] 每日待办智能继承
- [x] 置顶、透明度、暗色模式
- [x] 系统托盘与开机自启
- [x] 自动保存与窗口位置记忆
- [ ] 自定义快捷键
- [ ] 云同步 / WebDAV 备份
- [ ] 标签与搜索
- [ ] 导出为 PDF / HTML
- [ ] 插件扩展机制

---

## 更新日志

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
