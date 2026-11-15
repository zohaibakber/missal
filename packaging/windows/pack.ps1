param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Output = "dist-native"
)
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "../..")
Set-Location $root

$stage = Join-Path $root "target/stage-win"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item "target/release/missal.exe" $stage
Copy-Item "packaging/THIRD-PARTY-NOTICES.txt" $stage
Copy-Item "assets/fonts/Inter-LICENSE.txt" $stage
& (Join-Path $PSScriptRoot "fetch-sumatra.ps1") -Destination $stage

vpk pack `
    --packId Missal `
    --packVersion $Version `
    --packDir $stage `
    --mainExe missal.exe `
    --packTitle Missal `
    --packAuthors "Zohaib Akber" `
    --icon "assets/icon.ico" `
    --channel win `
    --outputDir $Output
if ($LASTEXITCODE -ne 0) { throw "vpk pack failed" }
