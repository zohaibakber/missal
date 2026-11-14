param(
    [Parameter(Mandatory = $true)][string]$Destination
)
$ErrorActionPreference = "Stop"

$lock = @{}
Get-Content (Join-Path $PSScriptRoot "sumatra.lock") | ForEach-Object {
    $key, $value = $_ -split "=", 2
    if ($key) { $lock[$key.Trim()] = "$value".Trim() }
}

if (-not $lock["sha256"] -or -not $lock["url"] -or -not $lock["version"]) {
    Write-Host "sumatra.lock is empty: SumatraPDF.exe is not bundled. Printing falls back to the default PDF viewer."
    exit 0
}

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("sumatra-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
$archive = Join-Path $work "sumatra.zip"
Invoke-WebRequest -Uri $lock["url"] -OutFile $archive

$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
if ($actual -ne $lock["sha256"].ToLowerInvariant()) {
    throw "SumatraPDF checksum mismatch: expected $($lock['sha256']), got $actual"
}

$extracted = Join-Path $work "x"
Expand-Archive -Path $archive -DestinationPath $extracted
$exe = Get-ChildItem -Path $extracted -Filter "*.exe" -Recurse | Select-Object -First 1
if (-not $exe) { throw "no executable inside the SumatraPDF archive" }
Copy-Item $exe.FullName (Join-Path $Destination "SumatraPDF.exe")
Write-Host "Bundled SumatraPDF $($lock['version']) ($actual)"
