#!/usr/bin/env bash
# Rust 命令提示（PreToolUse / Bash）：仅当本机确实找不到 MSVC 工具链时才提示。
# rustc 是通过 vswhere / 注册表定位 link.exe 的，找不到才会退回 PATH（在 Git Bash 里
# 就会命中 MSYS 的 usr/bin/link.exe，报 "link: extra operand"）。
# 只提示、不阻断（exit 0）。
set -u

cmd=$(jq -r '.tool_input.command // empty')
[ -z "$cmd" ] && exit 0

# 只关心 cargo / tauri 相关调用
printf '%s' "$cmd" | grep -qE \
	'(^|[;&|[:space:]])(cargo([[:space:]]|$)|npm[[:space:]]+run[[:space:]]+tauri|npx[[:space:]]+tauri|pnpm[[:space:]]+tauri|yarn[[:space:]]+tauri)' ||
	exit 0

VSWHERE="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
if [ -x "$VSWHERE" ] &&
	"$VSWHERE" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>/dev/null |
	grep -q .; then
	exit 0 # 工具链在，放行
fi

printf '%s' "[rust-shell-guard] 本机当前找不到 MSVC C++ 工具链（vswhere 缺失或没有 VC.Tools.x86.x64 组件），所以这条命令跑不通：PowerShell 报 linker 'link.exe' not found；Git Bash 会退回 PATH 上的 usr/bin/link.exe 并报 link: extra operand。这不是代码问题，别反复重试或改代码绕。修复：安装 Build Tools for Visual Studio 2022 的 Microsoft.VisualStudio.Workload.VCTools 负载（详见 AGENTS.local.md）。工具链恢复后本提示会自动消失。本会话可自查的仍是 npx tsc --noEmit -p tsconfig.json 与 npm run build。" |
	jq -Rs '{hookSpecificOutput:{hookEventName:"PreToolUse", additionalContext:.}}'
exit 0
