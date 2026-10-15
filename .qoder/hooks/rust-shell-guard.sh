#!/usr/bin/env bash
# Rust 命令提示（PreToolUse / Bash）：Git Bash 里 MSVC 链接器被 Git 的 link.exe 遮蔽，
# cargo / tauri 相关命令必然链接失败。只提示、不阻断（exit 0）。
set -u

cmd=$(jq -r '.tool_input.command // empty')
[ -z "$cmd" ] && exit 0

if printf '%s' "$cmd" | grep -qE '(^|[;&|[:space:]])(cargo([[:space:]]|$)|npm[[:space:]]+run[[:space:]]+tauri|npx[[:space:]]+tauri|pnpm[[:space:]]+tauri|yarn[[:space:]]+tauri)'; then
  printf '%s' "[rust-shell-guard] 这条命令在当前 Git Bash 环境跑不通：C:\Program Files\Git\usr\bin\link.exe 会遮蔽 MSVC 的 link.exe，rustc 链接阶段报 'link: extra operand'。cargo test / npm run tauri dev|build 请在 IDE 内置终端执行（Rust 单测在 src-tauri/src/lib.rs 的 #[cfg(test)] mod tests）。本会话内可用的校验是：npx tsc --noEmit -p tsconfig.json。" \
    | jq -Rs '{hookSpecificOutput:{hookEventName:"PreToolUse", additionalContext:.}}'
fi
exit 0
