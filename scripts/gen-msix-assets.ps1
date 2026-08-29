# Generates the MSIX tile/store icon assets from the app logo.
#
# System.Drawing bicubic scaling of crates/limedl-native/ui/assets/logo.png
# into every size AppxManifest references. Runs on windows-latest CI and locally.
#
# Usage: pwsh scripts/gen-msix-assets.ps1 -OutDir packaging/msix/staging/Assets

param(
    [string]$Source = (Join-Path $PSScriptRoot "..\crates\limedl-native\ui\assets\logo.png"),
    [Parameter(Mandatory = $true)]
    [string]$OutDir
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $Source)) {
    throw "Source logo not found: $Source"
}

Add-Type -AssemblyName System.Drawing

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$sizes = [ordered]@{
    "Square44x44Logo.targetsize-44"    = 44
    "Square44x44Logo.targetsize-66"    = 66
    "Square44x44Logo.targetsize-88"    = 88
    "Square44x44Logo.targetsize-176"   = 176
    "Square150x150Logo.scale-100"      = 150
    "Square150x150Logo.scale-125"      = 188
    "Square150x150Logo.scale-150"      = 225
    "Square150x150Logo.scale-200"      = 300
    "StoreLogo.scale-100"              = 50
}

$src = [System.Drawing.Image]::FromFile((Resolve-Path $Source))
try {
    foreach ($name in $sizes.Keys) {
        $size = [int]$sizes[$name]
        $bmp = New-Object System.Drawing.Bitmap($size, $size)
        try {
            $g = [System.Drawing.Graphics]::FromImage($bmp)
            try {
                $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
                $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
                $g.DrawImage($src, 0, 0, $size, $size)
            }
            finally {
                $g.Dispose()
            }
            $outPath = Join-Path $OutDir "$name.png"
            $bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
            Write-Host "  wrote $outPath"
        }
        finally {
            $bmp.Dispose()
        }
    }
}
finally {
    $src.Dispose()
}

Write-Host "MSIX assets generated in $OutDir"
