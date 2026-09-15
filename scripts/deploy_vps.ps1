# Deploy source bsc-sandwich len VPS qua SSH tu may Windows.
# Dung khi da CO SSH access toi VPS (key file) - script nay KHONG BAO GIO
# nhan/luu password qua tham so.
#
# Usage:
#   .\scripts\deploy_vps.ps1 -VpsHost <ip> [-VpsUser root] [-Port 22] `
#     [-Identity C:\path\key] [-RemotePath /root/bsc-sandwich] `
#     [-Probe] [-Build] [-Run]
#
# -Probe  sau khi copy, chay scripts/run_rpc_probe.sh TREN VPS (can .env
#         tren VPS co BSC_HTTP/BSC_WS that)
# -Build  sau khi copy, chay `cargo build --release` TREN VPS
# -Run    sau build, chay bot NGAM TREN VPS (dry_run theo config.toml da copy)
param(
    [Parameter(Mandatory = $true)] [string]$VpsHost,
    [string]$VpsUser = "root",
    [string]$Port = "22",
    [string]$Identity = "",
    [string]$RemotePath = "/root/bsc-sandwich",
    [switch]$Probe,
    [switch]$Build,
    [switch]$Run
)

$ErrorActionPreference = "Stop"

function Resolve-SshTool([string]$name) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $gitBundled = "C:\Program Files\Git\usr\bin\$name.exe"
    if (Test-Path $gitBundled) { return $gitBundled }
    return $null
}

$sshExe = Resolve-SshTool "ssh"
$scpExe = Resolve-SshTool "scp"

if (-not $sshExe -or -not $scpExe) {
    Write-Host "MISSING: khong tim thay ssh.exe/scp.exe (khong o PATH, khong o Git for Windows)."
    Write-Host "Cai OpenSSH Client (chay PowerShell voi quyen Administrator):"
    Write-Host "  Add-WindowsCapability -Online -Name OpenSSH.Client~~~~0.0.1.0"
    Write-Host "Hoac cai Git for Windows (da kem san ssh.exe/scp.exe o usr\bin)."
    Write-Host "Hoac dung WinSCP (GUI) de copy thu cong roi SSH tay chay lenh trong README.md."
    exit 1
}

Write-Host "Dung ssh: $sshExe"
Write-Host "Dung scp: $scpExe"

Set-Location (Join-Path $PSScriptRoot "..")

$sshArgs = @("-o", "BatchMode=yes", "-o", "ConnectTimeout=8", "-p", $Port)
if ($Identity -ne "") { $sshArgs += @("-i", $Identity) }
$target = "$VpsUser@$VpsHost"

Write-Host "== 1/4: kiem tra SSH toi $target port $Port (BatchMode - khong hoi password) =="
& $sshExe @sshArgs $target "echo ok"
if ($LASTEXITCODE -ne 0) {
    Write-Host "LOI: khong SSH duoc bang key hien co. Kiem tra -Identity." -ForegroundColor Red
    exit 1
}

Write-Host "== 2/4: dam bao thu muc dich + rustup tren VPS =="
& $sshExe @sshArgs $target "mkdir -p '$RemotePath'"
$rustupCheck = 'if ! command -v cargo >/dev/null 2>&1; then echo "rustup chua co tren VPS, cai (khong hoi, stable, -y)..."; curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable; else echo "cargo da co tren VPS, bo qua cai rustup."; fi'
& $sshExe @sshArgs $target $rustupCheck

Write-Host "== 3/4: dong goi source (tar.exe Windows) roi scp len VPS =="
$tempTar = Join-Path $env:TEMP "bsc-sandwich-deploy.tar.gz"
if (Test-Path $tempTar) { Remove-Item $tempTar -Force }
$tarExe = (Get-Command tar -ErrorAction Stop).Source
& $tarExe --exclude=".git" --exclude="target" --exclude="state" --exclude="logs" `
    --exclude="artifacts" --exclude=".env" -czf $tempTar .
if ($LASTEXITCODE -ne 0) { throw "tar dong goi that bai" }

$remoteTar = "/tmp/bsc-sandwich-deploy.tar.gz"
$scpArgs = @("-o", "BatchMode=yes", "-P", $Port)
if ($Identity -ne "") { $scpArgs += @("-i", $Identity) }
& $scpExe @scpArgs $tempTar "${target}:${remoteTar}"
if ($LASTEXITCODE -ne 0) { throw "scp copy that bai" }

& $sshExe @sshArgs $target "tar -xzf '$remoteTar' -C '$RemotePath' && rm -f '$remoteTar'"
Remove-Item $tempTar -Force
Write-Host "da copy xong vao ${target}:${RemotePath}"

Write-Host "== 4/4: buoc tuy chon (-Probe/-Build/-Run) =="

if ($Probe) {
    Write-Host "-- rpc_probe TREN VPS (can .env tren VPS co BSC_HTTP/BSC_WS that) --"
    & $sshExe @sshArgs $target "cd '$RemotePath' && source `$HOME/.cargo/env && bash scripts/run_rpc_probe.sh"
}

if ($Build) {
    Write-Host "-- cargo build --release TREN VPS --"
    & $sshExe @sshArgs $target "cd '$RemotePath' && source `$HOME/.cargo/env && cargo build --release"
}

if ($Run) {
    Write-Host "-- chay bot NGAM TREN VPS (dry_run theo config.toml da copy) --"
    # Phat hien tu phien deploy-vps-live (xem docs/STATE.md): "nohup ... &
    # disown" qua exec_command SSH khong-tty (paramiko, kha nang ca ssh.exe
    # thuong khi goi qua kenh khong tty) TREO ca kenh SSH vi shell cha khong
    # bao gio thoat duoc du tien trinh con da chay nen. Doi sang
    # `systemd-run --collect` - tra shell ve NGAY, khong phu thuoc tty/kenh
    # SSH con song hay khong.
    $runCmd = "cd '$RemotePath' && if command -v systemd-run >/dev/null 2>&1; then " +
        "systemd-run --unit=bsc-sandwich-paper --working-directory='$RemotePath' --collect " +
        "--property=StandardOutput=append:/root/bsc-sandwich-run.log " +
        "--property=StandardError=append:/root/bsc-sandwich-run.log " +
        "/bin/bash -c 'set -a; [ -f .env ] && source .env; set +a; exec ./target/release/bsc_sandwich'; " +
        "else echo 'CANH BAO: khong tim thay systemd-run tren VPS nay, fallback nohup (co the treo kenh SSH neu goi qua kenh non-tty).'; " +
        "nohup ./target/release/bsc_sandwich > /root/bsc-sandwich-run.log 2>&1 & disown; fi; " +
        "sleep 1; curl -s http://127.0.0.1:8787/api/status || true; echo"
    & $sshExe @sshArgs $target $runCmd
}

Write-Host "Xong. Xem dashboard tu may ban qua tunnel (KHONG mo port 8787 public):"
Write-Host "  ssh -N -L 8787:127.0.0.1:8787 -p $Port $target"
