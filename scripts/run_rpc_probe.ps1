# Cum `rpc-probe`. Nap .env (neu co) vao bien moi truong cua TIEN TRINH nay
# roi chay `cargo run --bin rpc_probe --release` - KHONG in noi dung .env ra
# man hinh, KHONG sua .env. Dung tren may Windows (vd may dev local).
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$envPath = ".env"
if (Test-Path $envPath) {
    Get-Content $envPath | ForEach-Object {
        $line = $_.Trim()
        if ($line -eq "" -or $line.StartsWith("#")) { return }
        $idx = $line.IndexOf("=")
        if ($idx -lt 1) { return }
        $key = $line.Substring(0, $idx).Trim()
        $val = $line.Substring($idx + 1).Trim()
        Set-Item -Path "Env:$key" -Value $val
    }
} else {
    Write-Host "MISSING: khong thay file .env o $(Get-Location) (chi anh huong bien moi truong, khong tu tao .env)"
}

cargo run --bin rpc_probe --release
