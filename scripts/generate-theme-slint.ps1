# Generates ui/theme.slint from the color mapping tables below.
#  - $map: static tokens, `dark ? <literal> : <light>` (dark = the exact
#    literal used across the UI, so dark mode stays pixel-identical).
#    Values of $null mean "unchanged in light mode; keep as literal".
#  - $accentMap: brand-accent tokens that follow settings.appearance.theme_color
#    (lime/amber/sky). Each entry maps the dark literal to per-accent values
#    for both schemes. The UI literals for these keys are replaced with
#    `Theme.c<hex>` by apply-theme-tokens.ps1, same as static tokens.
$ErrorActionPreference = "Stop"

$map = [ordered]@{
    # window / panel / card surfaces
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
    # text
    "f3f4f6" = "1f2329"; "d1d5db" = "374151"; "9ca3af" = "6b7280"
    "6b7280" = "6b7280"; "8b95a5" = "7c8698"; "5d6778" = "8a93a3"
    "4b5563" = "9ca3af"; "626c7d" = "8a93a3"; "e5e7eb" = "374151"
    "ffffff" = "1f2329"
    # info / blue (semantic, not brand accent)
    "38bdf8" = "0284c7"; "60a5fa" = "2563eb"; "112842" = "dbeafe"; "1d406b" = "bfdbfe"; "132a42" = "e0f2fe"
    "255078" = "93c5fd"; "152c42" = "e0f2fe"
    # success / status green (semantic, not brand accent)
    "4ade80" = "16a34a"; "22c55e" = "16a34a"; "86efac" = "15803d"
    "142e1b" = "dcfce7"; "14351a" = "dcfce7"; "153920" = "dcfce7"
    "22542e" = "bbf7d0"; "276238" = "15803d"
    # danger / red
    "ef4444" = "dc2626"; "f87171" = "dc2626"
    "991b1b" = "b91c1c"; "7f1d1d" = "b91c1c"
    "201315" = "fef2f2"; "361214" = "fee2e2"; "3f1518" = "fee2e2"
    "3b1216" = "fee2e2"; "5c1f23" = "fecaca"; "351417" = "fee2e2"
    "fca5a5" = "f87171"
    # warning / amber (semantic)
    "f59e0b" = "d97706"; "facc15" = "eab308"; "fcd34d" = "fbbf24"
    "fbbf24" = "d97706"; "351d0b" = "fef3c7"; "3b280c" = "fef3c7"
    "543615" = "fde68a"; "33200b" = "fde68a"; "2a1d08" = "fef9c3"
    # pink (overclock accents)
    "ec4899" = "db2777"; "f472b6" = "db2777"; "31132b" = "fce7f3"
    # overlays
    "000000bb" = "00000066"; "000000cc" = "00000066"
}

# Brand accent tokens: settings.appearance.theme_color (lime/amber/sky).
# Keys are the dark literals found in the UI. Per-accent values for dark and
# light schemes (amber follows Tailwind amber, sky follows Tailwind sky).
$accentMap = [ordered]@{
    "84cc16" = @{ # main accent (buttons, logo, selected, badges)
        dark  = @{ lime = "84cc16"; amber = "f59e0b"; sky = "0ea5e9" }
        light = @{ lime = "65a30d"; amber = "d97706"; sky = "0284c7" }
    }
    "a3e635" = @{ # bright accent (hover, brand text)
        dark  = @{ lime = "a3e635"; amber = "fbbf24"; sky = "38bdf8" }
        light = @{ lime = "84cc16"; amber = "f59e0b"; sky = "0ea5e9" }
    }
    "65a30d" = @{ # pressed accent
        dark  = @{ lime = "65a30d"; amber = "d97706"; sky = "0284c7" }
        light = @{ lime = "4d7c0f"; amber = "b45309"; sky = "0369a1" }
    }
    "365314" = @{ # deep active background
        dark  = @{ lime = "365314"; amber = "78350f"; sky = "0c4a6e" }
        light = @{ lime = "1a2e05"; amber = "713f12"; sky = "0c4a6e" }
    }
    "26331a" = @{ # accent tint background
        dark  = @{ lime = "26331a"; amber = "451a03"; sky = "082f49" }
        light = @{ lime = "ecfccb"; amber = "fef3c7"; sky = "e0f2fe" }
    }
    "232b1d" = @{ # soft accent tint background
        dark  = @{ lime = "232b1d"; amber = "3b280c"; sky = "0c4a6e" }
        light = @{ lime = "f0f7e2"; amber = "fef3c7"; sky = "e0f2fe" }
    }
    "2a3322" = @{ # mid accent tint background
        dark  = @{ lime = "2a3322"; amber = "422006"; sky = "075985" }
        light = @{ lime = "eaf6d9"; amber = "fde68a"; sky = "bae6fd" }
    }
    "1c2618" = @{ # strong accent tint background
        dark  = @{ lime = "1c2618"; amber = "451a03"; sky = "082f49" }
        light = @{ lime = "eaf6d9"; amber = "fef3c7"; sky = "e0f2fe" }
    }
    "4d5c41" = @{ # muted accent border/text
        dark  = @{ lime = "4d5c41"; amber = "92400e"; sky = "0369a1" }
        light = @{ lime = "9db876"; amber = "d97706"; sky = "0284c7" }
    }
}
# Colors with identical values in both schemes stay as literals in the UI:
$unchanged = @("ffffff", "dc2626", "b91c1c", "fecaca", "fef2f2", "0b1104", "6b7280")

$props = New-Object System.Text.StringBuilder
foreach ($entry in $map.GetEnumerator()) {
    $hex = $entry.Key
    $light = $entry.Value
    if ($null -eq $light) { continue }
    [void]$props.AppendLine("    /// dark #$hex / light #$light")
    [void]$props.AppendLine("    in property <color> c${hex}: dark ? #${hex} : #${light};")
}
foreach ($entry in $accentMap.GetEnumerator()) {
    $hex = $entry.Key
    $a = $entry.Value
    $darkExpr = "accent == ThemeAccent.Amber ? #$(($a.dark.amber)) : accent == ThemeAccent.Sky ? #$(($a.dark.sky)) : #$(($a.dark.lime))"
    $lightExpr = "accent == ThemeAccent.Amber ? #$(($a.light.amber)) : accent == ThemeAccent.Sky ? #$(($a.light.sky)) : #$(($a.light.lime))"
    [void]$props.AppendLine("    /// brand accent (was dark #$hex) — follows theme_color")
    [void]$props.AppendLine("    in property <color> c${hex}: dark ? ($darkExpr) : ($lightExpr);")
}

$header = @"
// Auto-generated color theme for the native UI. See scripts/generate-theme-slint.ps1
// for the source mapping tables (static dark -> light values + brand accent ramps).
//
// Usage rules:
//  - 'mode' is set from Rust (settings.appearance.color_mode).
//  - 'accent' is set from Rust (settings.appearance.theme_color).
//  - 'dark' resolves the effective scheme: explicit choice, or the OS scheme
//    in system mode (via the std-widgets Palette global).
//  - Property names are the DARK hex values (c<hex>) so every call site maps
//    1:1 back to the literal it replaced. Do not introduce new hardcoded
//    hex colors in components; add a token here instead. Brand-accent tokens
//    (buttons/selection/brand text) follow 'accent'; status colors
//    (success/warning/danger/info) intentionally do not.
import { Palette } from "std-widgets.slint";

export enum ColorModePref { System, Light, Dark }

export enum ThemeAccent { Lime, Amber, Sky }

export global Theme {
    /// User preference from settings (system/light/dark).
    in-out property <ColorModePref> mode: ColorModePref.System;

    /// Brand accent from settings (lime/amber/sky).
    in-out property <ThemeAccent> accent: ThemeAccent.Lime;

    /// Effective dark flag used by every color token below.
    // Note: builtin ColorScheme enum uses lowercase variants.
    in-out property <bool> dark: mode == ColorModePref.Dark
        || (mode == ColorModePref.System && Palette.color-scheme == ColorScheme.dark);

"@

$footer = @"
}
"@

$content = $header + "`n" + $props.ToString() + $footer + "`n"
[System.IO.File]::WriteAllText("D:\limedl\crates\limedl-native\ui\theme.slint", $content, [System.Text.UTF8Encoding]::new($false))
Write-Host "theme.slint written: $((Get-Content D:\limedl\crates\limedl-native\ui\theme.slint).Count) lines"
