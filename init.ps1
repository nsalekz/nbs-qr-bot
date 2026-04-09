# NBS QR Bot - Dev Environment Init
# Run: .\init.ps1

Write-Host "Building nbs-qr-bot..." -ForegroundColor Cyan
cargo build --release

if ($LASTEXITCODE -eq 0) {
    Write-Host "Build successful." -ForegroundColor Green
    Write-Host ""
    Write-Host "Start bot:" -ForegroundColor Yellow
    Write-Host '  $env:TELOXIDE_TOKEN="YOUR_TOKEN"; .\target\release\nbs-qr-bot.exe bot'
    Write-Host ""
    Write-Host "CLI usage:" -ForegroundColor Yellow
    Write-Host '  .\target\release\nbs-qr-bot.exe gen "paste invoice dump here" -o qr.png'
} else {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}
