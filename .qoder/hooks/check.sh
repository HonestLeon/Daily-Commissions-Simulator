#!/usr/bin/env bash
# 收尾自检（Stop 事件）：tsc 类型检查 + 版本号四处一致 + 文档同步提醒 + build-release.ps1 编码（BOM/LF）。
# 只提示、不阻断：始终以 0 退出，报告通过 additionalContext 注入对话。
set -u

msg=""
note() { msg="${msg}$1"$'\n'; }

# ── 1. 前端类型检查（本仓库唯一的 lint/类型闸门：tsc，strict + noUnused*）──
if [ -d node_modules ] && command -v npx >/dev/null 2>&1; then
  if ! tsc_out=$(npx tsc --noEmit -p tsconfig.json 2>&1); then
    note "[tsc] 类型检查未通过："
    note "$(printf '%s\n' "$tsc_out" | head -20)"
  fi
else
  note "[tsc] 跳过：缺少 node_modules 或 npx（先 npm install）"
fi

# ── 2. 版本号一致性：以 package.json 为基准 ──
v_pkg=$(jq -r '.version // empty' package.json 2>/dev/null)
if [ -n "$v_pkg" ] && [ -f src-tauri/Cargo.toml ]; then
  v_cargo=$(grep -m1 '^version' src-tauri/Cargo.toml | cut -d'"' -f2)
  v_conf=$(jq -r '.version // empty' src-tauri/tauri.conf.json 2>/dev/null)
  v_lock=""
  if [ -f src-tauri/Cargo.lock ]; then
    v_lock=$(awk '/^name = "lightweight-sticky-notes"$/{f=1; next} f && /^version =/{print $3; exit}' \
      src-tauri/Cargo.lock | tr -d '"')
  fi

  drift=""
  [ -n "$v_cargo" ] && [ "$v_cargo" != "$v_pkg" ] && drift="${drift} Cargo.toml=$v_cargo"
  [ -n "$v_conf" ] && [ "$v_conf" != "$v_pkg" ] && drift="${drift} tauri.conf.json=$v_conf"
  [ -n "$v_lock" ] && [ "$v_lock" != "$v_pkg" ] && drift="${drift} Cargo.lock=$v_lock"
  [ -f src-tauri/Cargo.lock ] && [ -z "$v_lock" ] && drift="${drift} Cargo.lock 里没有 lightweight-sticky-notes 条目"
  if [ -n "$drift" ]; then
    note "[版本] package.json=$v_pkg，以下不一致:$drift"
  fi
fi

# ── 3. 改了代码但文档没动 ──
changed=$(git -c core.quotepath=false diff --name-only HEAD 2>/dev/null
  git -c core.quotepath=false ls-files --others --exclude-standard 2>/dev/null)
if printf '%s\n' "$changed" | grep -qE '^src/|^src-tauri/src/'; then
  if ! printf '%s\n' "$changed" | grep -qE '^(README\.md|docs/.*\.md)$'; then
    note "[文档] 有代码改动但 README.md / docs/*.md 未同步：新功能要补 README「更新日志」，影响使用则更新 docs/使用说明.md。"
  fi
fi

# ── 4. build-release.ps1 编码闸门：UTF-8 BOM + 全 LF ──
if [ -f build-release.ps1 ]; then
  bom=$(od -An -tx1 -N3 build-release.ps1 | tr -dc '0-9a-f')
  [ "$bom" = "efbbbf" ] || note "[编码] build-release.ps1 缺 UTF-8 BOM（首三字节=$bom）：Windows PowerShell 5.1 会按 GBK 解析，脚本里的中文与中文文件名校验全乱。只能局部 Edit，别整文件重写。"
  ncr=$(tr -dc '\r' < build-release.ps1 | wc -c | tr -dc '0-9')
  [ "${ncr:-0}" = "0" ] || note "[编码] build-release.ps1 有 $ncr 个 CR（$PWD）：仓库无 .gitattributes、core.autocrlf=false，全仓库文本均为 LF。"
fi

if [ -n "$msg" ]; then
  printf '%s' "$msg" | jq -Rs '{hookSpecificOutput:{hookEventName:"Stop", additionalContext:.}}'
fi
exit 0
