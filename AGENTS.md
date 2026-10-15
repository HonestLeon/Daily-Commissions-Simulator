# AGENTS.md

This file provides guidance to the AI agent when working with code in this repository.

「每日委托模拟器」：Windows 半透明 Markdown 桌面便签。Tauri 2 (Rust) + React 19 + TypeScript 5.8 + Vite 6 + Milkdown Crepe + Zustand，纯 CSS 无 UI 框架。

## 命令

- `npm run build` — 唯一的自动化正确性闸门（`tsc && vite build`）。快路径：`npx tsc --noEmit -p tsconfig.json`（约 3s）。没有 ESLint，也没有 CI；类型检查就是 lint。`noUnusedLocals`/`noUnusedParameters` 已开启，多余的 import 会直接让构建失败。
- `npm run format` / `npm run format:check` — Prettier（无 ESLint）。配置：`printWidth 100`、`trailingComma es5`、双引号、2 空格、`endOfLine lf`；`.prettierignore` 排除了 lock/产物/工具本地配置。
- `cargo test`（在 `src-tauri/`）— Rust 单元测试全部内联在 `src-tauri/src/lib.rs` 的 `#[cfg(test)] mod tests`。
  ⚠ 这个仓库的 Rust 构建/测试**由用户在 IDE 内置终端执行**：本 CLI 的 Git Bash 里 `cargo build|test` 与 `npm run tauri dev|build` 必定链接失败（`C:\Program Files\Git\usr\bin\link.exe` 遮蔽 MSVC 的 link.exe，报 `link: extra operand`）。不要把它当成代码问题去排查、也不要反复重试；你在会话内能自查的只有前端 `npx tsc --noEmit -p tsconfig.json`。需要验证 Rust 侧时，改完后请用户在他的终端跑一次。
- `npm run release` — 执行 `build-release.ps1`（`tauri build` + 组装），产出 gitignored 的 `发布包/`。`npm run release:doc` 加 `-SkipBuild`，复用 `src-tauri/target/release/` 里的现有产物，产物不存在会直接抛错。

## 已配置的 hooks（`.qoder/settings.json`，团队共享）

- `Stop` → `.qoder/hooks/check.sh`：收尾自检 = tsc 类型检查 + 版本号四处一致 + 「改了代码没改文档」提醒。只提示不阻断，报告以 `additionalContext` 回灌。
- `PreToolUse/Bash` → `.qoder/hooks/rust-shell-guard.sh`：命令确实是 `cargo …` / `npm run tauri …` 时提示改用 IDE 终端（已避开 `src-tauri/`、`Cargo.toml` 这类路径误报）。只提示不阻断。
- `PostToolUse/Edit|Write` → `.qoder/hooks/auto-format.sh`：对改动过的 ts/tsx/js/jsx/json/css 自动跑 Prettier。`*.md` 故意排除（中文表格与段落重排噪音大），需要时手动 `npm run format`。
- 要调整检查内容就改这三个脚本；别再往本文件里堆命令。

## 跨层同步的隐式契约（改一处必须改另一处）

- **版本号**：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 三处同值，外加 README「更新日志」一节。
- **依赖范围**：直接依赖一律 `~`（补丁级）+ lock 钉版本，别改回 `^`、别 `npm install pkg@latest`。剩余的 5 个 low 告警全部来自 `micromark-extension-math` 里嵌套的 `katex@0.16.x`（上游声明 `katex: ^0.16.0`，而修复版本只在 0.18.2+）——只能等上游放宽范围；**不要跑 `npm audit fix --force`**，它会把 Milkdown 降到 7.5.9 这种破坏性版本。
- **结算模块名**：Rust 侧 `DAILY_CONTENT` / `SETTLE_MODULES` 里的 `#### 必做`、`#### 学习/工作`、`#### 生活/其他` 按标题文本精确匹配，前端 `tomorrowCheck.ts` 的 `TOMORROW_ALLOWED_SECTIONS` 是同一份字符串的第二副本。
- **主题名**：`lib.rs` 的 `APP_THEMES` 与 `noteStore.ts` 的 `APP_THEMES` 各写了一份。
- 原石 / 摩拉 / 砺行连续周数以 Rust 为权威来源，前端只采纳后端回传的值，不再本地累加。

## 数据与运行时

- 用户数据在 `%APPDATA%\com.liurr.lightweight-sticky-notes\`（`config.json`、`notes_index.json`、`notes/<uuid>.md`）。dev 与正式版共用同一目录，所以 `npm run tauri dev` 会真实结算并改写用户当天的委托数据。
- `invoke` 的字段名用 camelCase（`newTitle`、`closeToTray`、`dataUrl`、`setupUrl`），Rust 命令签名是 snake_case，由 Tauri 自动转换。
- 更新检查打的是 Gitee：`repos/Honest_Leon/lightweight-sticky-notes/releases/latest`，`tag_name` 即版本号，附件文件名必须含 `setup` / `portable`。`git remote` 指向 GitHub（`Daily-Commissions-Simulator`），但发布物走 Gitee，且包名 / 数据目录标识 / Gitee 仓库名始终保持 `lightweight-sticky-notes`，不要「顺手改名」。
- 开机自启直接写 `HKCU\...\Run` 注册表，且 `cfg(debug_assertions)` 下被拒绝——只能在发布版验证该行为。
- 新用到任何窗口 / 插件 API（如 `allow-open-path`、`allow-set-always-on-top`）必须同时加进 `src-tauri/capabilities/default.json`，否则运行时被静默拒绝。

## Milkdown 编辑器的坑

- 空任务项必须写成 `- [ ] `（带尾空格），否则 Crepe 不渲染复选框；`ensure_task_trailing_space()` 在读写两侧都做规范化，改动内容流时别绕过它。
- 空白项（勾选框后无文本）在结算、周统计、继承中一律忽略；只含空白项的模块算**未完成**。
- Crepe 初始化时会回调一次 `markdownUpdated`，`NoteEditor.tsx` 的 `isFirstUpdate` 用于跳过这次伪保存——否则会刷新 `updated_at` 并把委托标题改成当天。
- 「明日委托」在 Markdown 里是引用块 `> 文本`，跨天时由 Rust 转成普通任务项。

## 风格

- 面向用户的文案、代码注释、commit message 全部用简体中文；commit 用 Conventional 前缀 + 中文描述（`feat: 新增摩拉币种系统，标题栏显示余额并结算`）。
- 不要自动 `git commit`——等用户明确说提交才动 git。工作区常年带着未提交的进行中改动，`git status` 里的 `M`/`??` 不一定是你的改动，别清理、别还原、别 `add -A`。
- 现有注释解释的是「为什么」（多数针对具体 bug），重构时不要顺手删掉。
- README「截图展示」用 `docs/assets/0*.png`（中文文件名，9 张），与 `docs/使用说明.md` 同源。`build-release.ps1` 会把 README 里的 `docs/assets/` 和 `src-tauri/icons/128x128.png` 改写成发布包内的 `assets/…`——改 README 图片路径时留意这条替换规则。
