#!/usr/bin/env bash
# PostToolUse(Edit|Write)：对刚改过的文件跑一次 Prettier。
# 只格式化、不阻断：任何失败都静默退出 0。
set -u

raw=$(jq -r '.tool_response.filePath // .tool_input.file_path // empty')
[ -z "$raw" ] && exit 0

# Git Bash 的 test/npx 不认反斜杠路径，先归一化
file=$(printf '%s' "$raw" | tr '\\' '/')

# 注意：*.md 故意不自动格式化（README/使用说明里的表格与中文段落重排会产生噪音 diff），
# 需要时手动 npm run format。
case "$file" in
  *.ts | *.tsx | *.js | *.jsx | *.json | *.css) ;;
  *) exit 0 ;;
esac
[ -f "$file" ] || exit 0

npx prettier --write "$file" >/dev/null 2>&1
exit 0
