# AGENTS.md

This file provides guidance to the AI agent when working with code in this repository.

「每日委托模拟器」：Windows 半透明 Markdown 桌面便签。Tauri 2 (Rust) + React 19 + TypeScript 5.8 + Vite 6 + Milkdown Crepe + Zustand，纯 CSS 无 UI 框架。

## 命令

- `npm run build` — 唯一的自动化正确性闸门（`tsc && vite build`）。快路径：`npx tsc --noEmit -p tsconfig.json`（约 3s）。没有 ESLint，也没有 CI；类型检查就是 lint。`noUnusedLocals`/`noUnusedParameters` 已开启，多余的 import 会直接让构建失败。
- `npm run format` / `npm run format:check` — Prettier（无 ESLint）。配置：`printWidth 100`、`trailingComma es5`、双引号、2 空格、`endOfLine lf`；`.prettierignore` 排除了 lock/产物/工具本地配置。
- `cargo test`（在 `src-tauri/`）— Rust 单元测试全部内联在 `src-tauri/src/lib.rs` 的 `#[cfg(test)] mod tests`。
  ⚠ Rust 在 Windows 上链接阶段**必须**有 MSVC 工具链 + Windows SDK（rustc 只编译，链接交给 `link.exe`）。二者由「Build Tools for Visual Studio 2022」的 `Microsoft.VisualStudio.Workload.VCTools` 负载提供，与用不用 VS Code 无关。rustc 靠 `…\Microsoft Visual Studio\Installer\vswhere.exe` + 注册表定位它们，找不到才退回 PATH——PATH 上的 `Git\usr\bin\link.exe`（MSYS 建硬链接小工具）会导致 `link: extra operand`，PowerShell 里则报 `linker 'link.exe' not found`。
  自检一行：`"/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath` 有输出即正常。本机 2026-10-15 已装回 Build Tools（MSVC 14.44.35207 + Windows SDK 10.0.26100），已验证 `cargo test` 23/23 通过、**在 Git Bash 里也能跑**（rustc 走 vswhere 定位，不会被 PATH 上的 MSYS `link.exe` 带偏）。缺了就按 AGENTS.local.md 的安装命令补，别改代码绕；`LNK1181: 无法打开 dbghelp.lib` 说明 SDK 的 `Lib\…\um\x64` 还没装齐（安装未完成或缺 Windows SDK 组件），不是代码问题。
- `npm run release` — 执行 `build-release.ps1`（`tauri build` + 组装），产出 gitignored 的 `发布包/`。`npm run release:doc` 加 `-SkipBuild`，复用 `src-tauri/target/release/` 里的现有产物，产物不存在会直接抛错。
  ⚠ `build-release.ps1` 是 **UTF-8 带 BOM**（首三字节 239,187,191），里面的中文文件名与产物校验全靠 BOM 才能被 Windows PowerShell 5.1 正确解析。改它只能用局部 Edit 保留 BOM，**不要整文件重写**——Write/Prettier 产出的是无 BOM UTF-8，脚本会被当 GBK 读，中文变乱码后语法直接崩（我踩过两次）。`.prettierignore` 不含 `.ps1`，Prettier 也不会处理它，别指望格式化工具帮你改写。

## 已配置的 hooks（`.qoder/settings.json`，团队共享）

- `Stop` → `.qoder/hooks/check.sh`：收尾自检 = tsc 类型检查 + 版本号四处一致 + 「改了代码没改文档」提醒。只提示不阻断，报告以 `additionalContext` 回灌。
- `PreToolUse/Bash` → `.qoder/hooks/rust-shell-guard.sh`：命令确实是 `cargo …` / `npm run tauri …` 时，**运行时用 vswhere 探测** MSVC 组件在不在，缺失才提示（工具链装好后自动闭嘴；已避开 `src-tauri/`、`Cargo.toml` 这类路径误报）。只提示不阻断。
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
