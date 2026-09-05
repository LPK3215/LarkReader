# ==============================================================================
# 飞书 lark-cli 一键安装与配置脚本
# ==============================================================================
# 用法: 在 PowerShell 中执行
#   powershell -ExecutionPolicy Bypass -File setup.ps1
# ==============================================================================

param(
    [string]$Brand = "feishu",        # feishu 或 lark
    [string]$Lang = "zh"              # 语言偏好
)

$ErrorActionPreference = "Stop"

function Write-Step($msg) { Write-Host "`n>>> $msg" -ForegroundColor Cyan }
function Write-OK($msg)   { Write-Host "    [OK] $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "    [!] $msg" -ForegroundColor Yellow }
function Write-Err($msg)  { Write-Host "    [X] $msg" -ForegroundColor Red }

# ── 0. 清除可能干扰的 Agent 环境变量 ──────────────────────────────
$env:HERMES_HOME = $null
$env:OPENCLAW_HOME = $null
$env:LARK_CHANNEL = $null
Write-OK "已清除 HERMES_HOME / OPENCLAW_HOME / LARK_CHANNEL 环境变量"

# ── 1. 检查 Node.js ──────────────────────────────────────────────
Write-Step "步骤 1/4: 检查 Node.js"
try {
    $nodeVer = node --version 2>&1
    if ($LASTEXITCODE -ne 0) { throw "not found" }
    Write-OK "Node.js $nodeVer"
} catch {
    Write-Err "Node.js 未安装！请先安装 Node.js 18+ : https://nodejs.org/"
    exit 1
}

# ── 2. 安装 lark-cli ──────────────────────────────────────────────
Write-Step "步骤 2/4: 安装/更新 lark-cli"
$larkCheck = lark-cli --version 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-OK "lark-cli 已安装: $larkCheck"
    $choice = Read-Host "    是否更新到最新版? (y/N)"
    if ($choice -eq "y" -or $choice -eq "Y") {
        npm install -g @larksuite/cli@latest 2>&1 | Out-Null
        Write-OK "已更新到最新版"
    }
} else {
    Write-Host "    正在安装 lark-cli..."
    npm install -g @larksuite/cli@latest 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Err "安装失败，请检查 npm 权限或网络"
        exit 1
    }
    $larkVer = lark-cli --version 2>&1
    Write-OK "lark-cli 安装成功: $larkVer"
}

# ── 3. 初始化飞书应用 ────────────────────────────────────────────
Write-Step "步骤 3/4: 初始化飞书应用配置"
$configShow = lark-cli config show 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-OK "应用已配置"
    $configShow | ConvertFrom-Json | Select-Object appId, brand | Format-Table
} else {
    Write-Host "    需要创建飞书应用，即将打开浏览器..."
    Write-Host "    请在浏览器中完成应用创建和配置"
    Write-Host ""
    lark-cli config init --new --brand $Brand --lang $Lang 2>&1 | ForEach-Object {
        Write-Host $_
        if ($_ -match "user_code=([A-Z0-9-]+)") {
            $code = $Matches[1]
        }
        if ($_ -match "(https://open\.feishu\.cn/page/cli\?[^ ]+)") {
            $url = $Matches[1]
            Write-Host ""
            Write-Host "    >>> 请在浏览器中打开以下链接 <<<" -ForegroundColor Yellow
            Write-Host "    $url" -ForegroundColor Yellow
            Write-Host ""
        }
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Err "应用配置失败"
        exit 1
    }
    Write-OK "应用配置成功"
}

# ── 4. 登录授权 ──────────────────────────────────────────────────
Write-Step "步骤 4/4: 用户登录授权"
$whoami = lark-cli whoami 2>&1
if ($LASTEXITCODE -eq 0 -and ($whoami -match '"tokenStatus":\s*"ready"')) {
    Write-OK "已登录，无需重复授权"
    $whoami | ConvertFrom-Json | Select-Object appId, brand, identity, tokenStatus | Format-Table
} else {
    Write-Host "    需要浏览器授权，即将打开链接..."
    Write-Host "    请在浏览器中完成飞书登录和授权"
    Write-Host ""
    lark-cli auth login --domain docs --domain drive --domain wiki 2>&1 | ForEach-Object {
        Write-Host $_
        if ($_ -match "(https://accounts\.feishu\.cn/oauth/v1/device/verify\?[^ ]+)") {
            $url = $Matches[1]
            Write-Host ""
            Write-Host "    >>> 请在浏览器中打开以下链接完成授权 <<<" -ForegroundColor Yellow
            Write-Host "    $url" -ForegroundColor Yellow
            Write-Host ""
        }
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Err "登录授权失败"
        exit 1
    }
    Write-OK "授权成功"
}

# ── 完成 ─────────────────────────────────────────────────────────
Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  全部配置完成！现在可以运行提取脚本了" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  用法:"
Write-Host "    python extract_generic.py <输出目录> `"node_token|标题`" `"node_token|标题`" ..."
Write-Host ""
Write-Host "  示例:"
Write-Host "    python extract_generic.py ./output `"Kh47wj3YRiPxsekidFWcbW0Knkb|Agent智能体知识库`""
Write-Host ""
Write-Host "  注意:"
Write-Host "    - 如果脚本报 hermes context 错误，在运行前执行:"
Write-Host "      `$env:HERMES_HOME = `$null"
Write-Host ""
