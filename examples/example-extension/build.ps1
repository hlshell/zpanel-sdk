# Zpanel 扩展构建和部署脚本
# 使用方法: .\build.ps1

param(
    [string]$OutputDir = "../../extend/dso",
    [switch]$Release
)

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Zpanel 扩展构建脚本" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# 检查 Cargo 命令
$cargoArgs = if ($Release) { "build --release" } else { "build" }
Write-Host "正在构建扩展..." -ForegroundColor Yellow
Invoke-Expression "cargo $cargoArgs"

if ($LASTEXITCODE -ne 0) {
    Write-Host "构建失败!" -ForegroundColor Red
    exit 1
}

# 确定目标路径
$targetDir = if ($Release) { "target/release" } else { "target/debug" }
$extensionName = "zpanel_example_extension"

# 根据平台确定文件名
$os = $ENV:OS
if ($os -like "*Windows*") {
    $extensionFile = "$extensionName.dll"
} elseif ($os -like "*Linux*") {
    $extensionFile = "lib$extensionName.so"
} else {
    $extensionFile = "lib$extensionName.dylib"
}

$sourcePath = Join-Path $targetDir $extensionFile

Write-Host "`n构建成功!" -ForegroundColor Green
Write-Host "扩展文件: $sourcePath"

# 创建输出目录
if (-not (Test-Path $OutputDir)) {
    Write-Host "创建输出目录: $OutputDir" -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

# 复制文件
$destPath = Join-Path $OutputDir $extensionFile
Write-Host "复制扩展文件到: $destPath" -ForegroundColor Yellow
Copy-Item $sourcePath $destPath -Force

# 复制配置文件
$configFile = "example_extension.conf"
if (Test-Path $configFile) {
    $destConfig = Join-Path $OutputDir $configFile
    Write-Host "复制配置文件到: $destConfig" -ForegroundColor Yellow
    Copy-Item $configFile $destConfig -Force
}

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  构建和部署完成!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "扩展已部署到: $OutputDir"
Write-Host "请重启 Zpanel 以加载新扩展" -ForegroundColor Yellow
