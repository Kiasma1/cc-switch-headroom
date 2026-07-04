# cc-switch dev 沙箱启动脚本
#
# 用独立 home 目录跑 dev 版,与正式版(和你的真实 ~/.claude)彻底隔离:
#   - 独立的 SQLite DB
#   - 独立的 <sandbox>/.claude/settings.json(dev 版接管只写这里,碰不到真实配置)
#
# 依据:cc-switch 的 config.rs get_home_dir() 会优先读 CC_SWITCH_TEST_HOME 环境变量。
#
# 用法(在 cc-switch 目录下):
#   .\scripts\dev-sandbox.ps1
#
# 首次启动后,若要测代理/压缩(需要代理跑起来),在应用里或控制台先把
# 代理端口从 15721 改成 15722,避免撞上正式版持有的 :15721:
#   （设置 → 代理 → 监听端口 改 15722,或控制台 update_global_proxy_config）
#
# 注意:代码里 Headroom 上游写死 :15721,所以沙箱里压缩的实际转发仍指向正式版 :15721。
# 对"测开关/状态/headroom 起停接线"够用;要测真实压缩流量需另把上游做成可配。

$ErrorActionPreference = "Stop"

$Sandbox = Join-Path $env:USERPROFILE "cc-switch-sandbox"
New-Item -ItemType Directory -Force -Path $Sandbox | Out-Null
# 确保沙箱 home 下有 ~/.claude 目录 + settings.json(接管/路由切换要读它,缺则报"Claude 配置文件不存在")
$ClaudeDir = Join-Path $Sandbox ".claude"
New-Item -ItemType Directory -Force -Path $ClaudeDir | Out-Null
$ClaudeSettings = Join-Path $ClaudeDir "settings.json"
if (-not (Test-Path $ClaudeSettings)) {
    "{}" | Set-Content -Path $ClaudeSettings -Encoding UTF8
    Write-Host "已在沙箱补种空的 .claude/settings.json" -ForegroundColor Green
}

$env:CC_SWITCH_TEST_HOME = $Sandbox

Write-Host "=== cc-switch dev 沙箱 ===" -ForegroundColor Cyan
Write-Host "CC_SWITCH_TEST_HOME = $Sandbox" -ForegroundColor Cyan
Write-Host "DB 与 settings 全在沙箱,不影响正式版与真实 ~/.claude。" -ForegroundColor Cyan
Write-Host "提醒:测代理/压缩前把监听端口改成 15722,避免撞正式版 :15721。" -ForegroundColor Yellow
Write-Host ""

pnpm tauri dev
