# Generates ui/theme.slint from the shared mapping tables in
# scripts/theme-tokens.ps1 — the same tables scripts/apply-theme-tokens.ps1 uses to
# rewrite UI literals, so the emitted tokens and the call sites can never disagree.
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "theme-tokens.ps1")

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
[System.IO.File]::WriteAllText($themeFile, $content, [System.Text.UTF8Encoding]::new($false))
Write-Host "theme.slint written: $((Get-Content $themeFile).Count) lines"
