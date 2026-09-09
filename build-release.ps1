# ============================================================
# 轻量便签 · 发布包构建脚本
# 用法:
#   powershell -ExecutionPolicy Bypass -File build-release.ps1            # 完整构建（tauri build + 打包）
#   powershell -ExecutionPolicy Bypass -File build-release.ps1 -SkipBuild # 仅刷新发布包（用现有构建产物）
#   或 npm run release / npm run release:doc
#
# 发布包结构（增量刷新）:
#   发布包/
#     README.md                      ← 复制自项目根
#     使用说明.md                    ← 复制自 docs/使用说明.md
#     assets/                        ← 复制自 docs/assets/
#     轻量便签_{ver}_x64-setup.exe    ← 安装版
#     轻量便签_{ver}_x64-portable.exe ← 免安装版
#     使用说明.pdf                   ★ 由人工转换放入，脚本不生成、不删除
#
# 注意: 脚本只删除/覆盖自己管理的文件（轻量便签_*.exe、README.md、
#       使用说明.md、assets/），用户手动放入的其他文件原样保留。
# ============================================================

param([switch]$SkipBuild)

$ErrorActionPreference = "Stop"

# 项目根 = 脚本所在目录
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

# ── 1. 读取版本号 ──
$pkg = Get-Content "package.json" -Raw -Encoding UTF8 | ConvertFrom-Json
$ver = $pkg.version
Write-Host "== 轻量便签发布包构建 v$ver =="

# ── 2. 构建（可跳过） ──
if (-not $SkipBuild) {
    Write-Host ""
    Write-Host "[1/4] 运行 tauri build ..."
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "tauri build 失败（退出码 $LASTEXITCODE）" }
} else {
    Write-Host ""
    Write-Host "[1/4] 跳过构建（-SkipBuild），使用现有产物"
}

# ── 3. 校验构建产物 ──
$setup = Join-Path $root "src-tauri\target\release\bundle\nsis\轻量便签_${ver}_x64-setup.exe"
$portableSrc = Join-Path $root "src-tauri\target\release\lightweight-sticky-notes.exe"
if (-not (Test-Path $setup))     { throw "未找到安装版产物: $setup" }
if (-not (Test-Path $portableSrc)) { throw "未找到免安装版产物: $portableSrc" }

# ── 4. 刷新发布包（增量，不清空整个目录） ──
$out = Join-Path $root "发布包"
if (-not (Test-Path $out)) { New-Item -ItemType Directory -Path $out | Out-Null }

Write-Host "[2/4] 清理旧版本 exe ..."
Get-ChildItem -Path $out -Filter "轻量便签_*.exe" -File -ErrorAction SilentlyContinue |
    Remove-Item -Force

Write-Host "[3/4] 复制文档与 assets ..."
Copy-Item (Join-Path $root "README.md")          $out -Force
Copy-Item (Join-Path $root "docs\使用说明.md")    $out -Force
$outAssets = Join-Path $out "assets"
if (Test-Path $outAssets) { Remove-Item $outAssets -Recurse -Force }
Copy-Item (Join-Path $root "docs\assets") $outAssets -Recurse

Write-Host "[4/4] 复制 exe ..."
Copy-Item $setup $out -Force
Copy-Item $portableSrc (Join-Path $out "轻量便签_${ver}_x64-portable.exe") -Force

# ── 5. 输出清单 ──
Write-Host ""
Write-Host "== 发布包内容 ($out) =="
Get-ChildItem $out -Recurse -File | ForEach-Object {
    $rel = $_.FullName.Substring($out.Length + 1)
    "{0,-52} {1,10:N0} KB" -f $rel, ($_.Length / 1KB)
}
Write-Host ""
Write-Host "完成。提示: 使用说明.pdf 请由 Markdown 手动转换后放入发布包，脚本不会覆盖或删除它。"
