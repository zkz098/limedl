# Replaces hardcoded hex color literals in ui/*.slint with Theme.<token> references.
# The dark -> light mapping lives in scripts/theme-tokens.ps1, shared with
# generate-theme-slint.ps1; tokens listed in $keepLiteral keep their literal value.
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "theme-tokens.ps1")

# Every token that has a Theme.c<hex> property, minus the keep-as-literal set.
# Longest hex first so 8-digit alphas replace before their 6-digit prefixes.
$keys = @(@($map.Keys) + @($accentMap.Keys) |
    Where-Object { $_ -notin $keepLiteral } |
    Sort-Object { $_.Length } -Descending)

foreach ($f in Get-ChildItem $uiDir -Recurse -Filter *.slint | Where-Object Name -ne "theme.slint") {
    $text = [System.IO.File]::ReadAllText($f.FullName)
    $original = $text
    foreach ($hex in $keys) {
        $text = [regex]::Replace($text, "#$hex(?![0-9a-fA-F])", "Theme.c$hex", [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    }
    if ($text -ne $original) {
        # inject the Theme import after the first std-widgets/types import block
        $rel = if ($f.Directory.Name -eq "components") { "../theme.slint" } else { "theme.slint" }
        if ($text -notmatch 'import \{ Theme') {
            # insert after the first line that imports from std-widgets or types
            $lines = $text -split "`n"
            $insertAt = 0
            for ($i = 0; $i -lt $lines.Count; $i++) {
                if ($lines[$i] -match '^(import .+ from )|(^export \{)') { $insertAt = $i + 1 }
                if ($i -gt 0 -and $lines[$i] -match '^import' -and $lines[$i-1] -notmatch '^import|^$|^\s') { break }
            }
            # simpler: insert before the first import statement, or at the top
            # when the file has none (avoid negative-range slicing bugs)
            $importLine = "import { Theme } from `"$rel`";"
            if ($insertAt -le 0) {
                $lines = @($importLine) + $lines
            } else {
                $lines = $lines[0..($insertAt-1)] + $importLine + $lines[$insertAt..($lines.Count-1)]
            }
            $text = $lines -join "`n"
        }
        [System.IO.File]::WriteAllText($f.FullName, $text, [System.Text.UTF8Encoding]::new($false))
        Write-Host "updated $($f.Name)"
    }
}

# report any remaining hex literals (must all be in the unchanged set)
$remaining = @{}
foreach ($f in Get-ChildItem $uiDir -Recurse -Filter *.slint | Where-Object Name -ne "theme.slint") {
    $text = [System.IO.File]::ReadAllText($f.FullName)
    foreach ($m in [regex]::Matches($text, '#([0-9a-fA-F]{3,8})\b')) {
        $h = $m.Groups[1].Value.ToLower()
        if ($h.Length -eq 6 -or $h.Length -eq 8) {
            if (-not $unchanged.Contains($h)) { $remaining["$h ($($f.Name))"] = $true }
        }
    }
}
if ($remaining.Count) {
    Write-Host "UNMAPPED colors remaining:"
    $remaining.Keys | ForEach-Object { "  $_" }
} else {
    Write-Host "all color literals accounted for"
}
