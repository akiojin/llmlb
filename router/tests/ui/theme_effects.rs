// Contract tests for theme system (FR-027)
// Simplified 3-theme system: Dark, Light, High Contrast
//
// Design principles:
// - Color-only themes (no layout changes per theme)
// - Fixed layout across all themes (Minimal-based)
// - No decorative visual effects (scanlines, glows, grids removed)
// - Accessibility support (prefers-reduced-motion, high-contrast)

/// Read the styles.css file content for testing
fn get_styles_css() -> String {
    include_str!("../../src/web/static/styles.css").to_string()
}

// ============================================
// THEME SYSTEM TESTS (3 Color Themes)
// ============================================

#[test]
fn dark_theme_is_default() {
    let css = get_styles_css();
    // Dark theme should be defined in :root (default)
    assert!(
        css.contains(":root") && css.contains("--bg:"),
        "Root should define default theme variables"
    );
    assert!(
        css.contains("[data-theme=\"dark\"]"),
        "Dark theme selector should exist"
    );
}

#[test]
fn light_theme_exists_with_light_colors() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"light\"]"),
        "Light theme selector should exist"
    );
    // Light theme should have color-scheme: light
    assert!(
        css.contains("color-scheme: light"),
        "Light theme should set color-scheme: light"
    );
}

#[test]
fn high_contrast_theme_exists() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"high-contrast\"]"),
        "High contrast theme selector should exist"
    );
}

#[test]
fn all_themes_define_core_css_variables() {
    let css = get_styles_css();
    // Core variables that must exist
    let core_vars = ["--bg", "--bg-card", "--text", "--accent", "--border"];

    for var in core_vars {
        assert!(
            css.contains(var),
            "CSS should define core variable: {}",
            var
        );
    }
}

#[test]
fn themes_use_css_custom_properties_for_colors() {
    let css = get_styles_css();
    // Verify color values are defined via CSS custom properties
    assert!(
        css.contains("var(--bg)") || css.contains("var(--bg-card)"),
        "CSS should use CSS custom properties for theming"
    );
}

// ============================================
// ACCESSIBILITY TESTS
// ============================================

#[test]
fn prefers_reduced_motion_is_respected() {
    let css = get_styles_css();
    assert!(
        css.contains("prefers-reduced-motion"),
        "CSS should respect prefers-reduced-motion for accessibility"
    );
}

#[test]
fn high_contrast_theme_has_pure_black_background() {
    let css = get_styles_css();
    // High contrast should use pure black (#000) for maximum contrast
    assert!(
        css.contains("[data-theme=\"high-contrast\"]") && css.contains("#000"),
        "High contrast theme should use pure black background"
    );
}

// ============================================
// LAYOUT TESTS (Fixed Layout)
// ============================================

#[test]
fn layout_is_theme_independent() {
    let css = get_styles_css();
    // Layout classes should not be theme-specific
    // (no [data-theme="x"] .stats-grid with different layouts)
    assert!(
        !css.contains("[data-theme=\"dark\"] .stats-grid"),
        "Dark theme should not have theme-specific layout"
    );
    assert!(
        !css.contains("[data-theme=\"light\"] .stats-grid"),
        "Light theme should not have theme-specific layout"
    );
}

#[test]
fn no_decorative_effects_exist() {
    let css = get_styles_css();
    // Decorative effects should be removed
    assert!(
        !css.contains("scanline"),
        "Scanline effects should be removed"
    );
    assert!(
        !css.contains("grid-overlay"),
        "Grid overlay effects should be removed"
    );
    assert!(!css.contains("crt-effect"), "CRT effects should be removed");
}

#[test]
fn no_legacy_themes_exist() {
    let css = get_styles_css();
    // Legacy 7-color themes should be removed
    let legacy_themes = [
        "synthwave",
        "ocean",
        "ember",
        "forest",
        "cyberpunk",
        "retro",
        "mono",
    ];

    for theme in legacy_themes {
        let selector = format!("[data-theme=\"{}\"]", theme);
        assert!(
            !css.contains(&selector),
            "Legacy theme '{}' should be removed",
            theme
        );
    }
}

#[test]
fn no_legacy_skin_themes_exist() {
    let css = get_styles_css();
    // Legacy 3-skin themes should be removed
    let legacy_skins = ["minimal", "tech", "creative"];

    for skin in legacy_skins {
        let selector = format!("[data-theme=\"{}\"]", skin);
        assert!(
            !css.contains(&selector),
            "Legacy skin theme '{}' should be removed",
            skin
        );
    }
}

// ============================================
// RESPONSIVE DESIGN TESTS
// ============================================

#[test]
fn responsive_breakpoints_exist() {
    let css = get_styles_css();
    assert!(
        css.contains("@media"),
        "CSS should have responsive media queries"
    );
}

#[test]
fn mobile_friendly_layout() {
    let css = get_styles_css();
    // Should have mobile-specific styles
    assert!(
        css.contains("max-width:") || css.contains("min-width:"),
        "CSS should have responsive breakpoints"
    );
}
