# Replaces hardcoded hex color literals in ui/*.slint with Theme.<token>
# references. The mapping (dark -> light) lives in generate-theme-slint.ps1;
# colors mapped to $null are intentionally left as literals (same value in
# both schemes).
$ErrorActionPreference = "Stop"

# Load the dark->light map (colors mapped to $null would be "unchanged", but
# none exist, so all keys are replaced).
$map = [ordered]@{
        "0d0f12" = "f4f5f7"; "12151a" = "ffffff"; "181b22" = "ffffff"
        "111419" = "f8f9fa"; "111418" = "f8f9fa"; "0b0c0f" = "e9ebef"
        "13161b" = "f2f3f5"; "14161b" = "f0f1f4"; "14171d" = "eff1f4"
        "141922" = "eef0f4"; "15171c" = "eff1f4"; "15181d" = "eff1f4"
        "15181e" = "eff1f4"; "15181f" = "eff1f4"; "151921" = "eef0f3"
        "171a20" = "f0f1f4"; "171a21" = "eff1f4"; "171b21" = "eef0f3"
        "191c22" = "eff1f4"; "1a1d23" = "eceef2"; "1a1e25" = "eceef2"
        "1a1e26" = "eceef1"; "1d212a" = "e9ecf0"; "1e2229" = "e9ecf0"
        "1f242e" = "e8ebf0"; "20242c" = "e7eaef"; "20242d" = "e7eaef"
        "20252e" = "e7eaef"; "22262f" = "e6e9ee"; "222731" = "e7eaef"
        "232832" = "e6e9ee"; "232a38" = "e6e9ee"; "242932" = "e6e9ee"; "242934" = "e6e9ee"
        "252a34" = "e5e8ee"; "252b34" = "e5e8ee"; "252b36" = "e5e8ee"
        "262a31" = "e5e8ed"; "262c37" = "e4e7ed"; "272d38" = "dfe3ea"
        "272f3e" = "dfe3ea"; "2a2f3a" = "dfe2e9"; "2b313d" = "dee2e9"
        "2b3342" = "dfe3ec"; "2e3542" = "d9dde5"; "323844" = "d5dae3"
        "323845" = "d5dae3"; "343c4a" = "d2d8e1"; "3b4352" = "cdd4df"
        "3f4756" = "c9d0da"; "202632" = "e4e7ed"; "181d26" = "eef0f4"
        "f3f4f6" = "1f2329"; "d1d5db" = "374151"; "9ca3af" = "6b7280"
        "8b95a5" = "7c8698"; "5d6778" = "8a93a3"; "4b5563" = "9ca3af"
        "626c7d" = "8a93a3"; "e5e7eb" = "374151"; "ffffff" = "1f2329"
        "84cc16" = "65a30d"; "a3e635" = "84cc16"; "65a30d" = "4d7c0f"
        "26331a" = "ecfccb"; "232b1d" = "f0f7e2"; "2a3322" = "eaf6d9"
        "4d5c41" = "9db876"; "365314" = "3f6212"; "1c2618" = "eaf6d9"
        "38bdf8" = "0284c7"; "60a5fa" = "2563eb"; "152c42" = "e0f2fe"
        "112842" = "dbeafe"; "1d406b" = "bfdbfe"; "132a42" = "e0f2fe"
        "255078" = "93c5fd"
        "4ade80" = "16a34a"; "22c55e" = "16a34a"; "86efac" = "15803d"
        "142e1b" = "dcfce7"; "14351a" = "dcfce7"; "153920" = "dcfce7"
        "22542e" = "bbf7d0"; "276238" = "15803d"
        "ef4444" = "dc2626"; "f87171" = "dc2626"
        "991b1b" = "b91c1c"; "7f1d1d" = "b91c1c"
        "201315" = "fef2f2"; "361214" = "fee2e2"; "3f1518" = "fee2e2"
        "3b1216" = "fee2e2"; "5c1f23" = "fecaca"; "351417" = "fee2e2"
        "fca5a5" = "f87171"
        "f59e0b" = "d97706"; "facc15" = "eab308"; "fbbf24" = "d97706"
        "fcd34d" = "fbbf24"; "351d0b" = "fef3c7"; "3b280c" = "fef3c7"
        "543615" = "fde68a"; "33200b" = "fde68a"; "2a1d08" = "fef9c3"
        "ec4899" = "db2777"; "f472b6" = "db2777"; "31132b" = "fce7f3"
        "000000bb" = "00000066"; "000000cc" = "00000066"
}

$uiDir = "D:\limedl\crates\limedl-native\ui"
# longest hex first so 8-digit alphas replace before their 6-digit prefixes
$keys = @($map.Keys | Sort-Object { $_.Length } -Descending)

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

# report any remaining hex literals (must all be in the "unchanged" set)
$allowed = @("ffffff", "dc2626", "b91c1c", "fecaca", "fef2f2", "0b1104", "6b7280")
$remaining = @{}
foreach ($f in Get-ChildItem $uiDir -Recurse -Filter *.slint | Where-Object Name -ne "theme.slint") {
    $text = [System.IO.File]::ReadAllText($f.FullName)
    foreach ($m in [regex]::Matches($text, '#([0-9a-fA-F]{3,8})\b')) {
        $h = $m.Groups[1].Value.ToLower()
        if ($h.Length -eq 6 -or $h.Length -eq 8) {
            if (-not $allowed.Contains($h)) { $remaining["$h ($($f.Name))"] = $true }
        }
    }
}
if ($remaining.Count) {
    Write-Host "UNMAPPED colors remaining:"
    $remaining.Keys | ForEach-Object { "  $_" }
} else {
    Write-Host "all color literals accounted for"
}
