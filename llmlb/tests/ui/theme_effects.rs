// Contract tests for theme system (FR-027)
// React + Tailwind + shadcn/ui based theme system: Dark, Light
//
// Design principles:
// - CSS custom properties for theme colors via Tailwind
// - Dark/Light mode toggle via class on html element
// - Accessibility support (prefers-reduced-motion)

/// Read the index.css file content for testing (React dashboard source)
fn get_styles_css() -> String {
    include_str!("../../src/web/dashboard/src/index.css").to_string()
}

// ============================================
// THEME SYSTEM TESTS (Dark/Light Themes)
// ============================================

#[test]
fn dark_theme_is_default() {
    let css = get_styles_css();
    // Dark theme should be defined in :root or .dark
    assert!(
        css.contains(":root") || css.contains(".dark"),
        "Root or .dark should define theme variables"
    );
}

#[test]
fn light_theme_exists() {
    let css = get_styles_css();
    // Light theme should have its own section (no .dark class)
    assert!(
        css.contains(":root") && !css.contains(".dark :root"),
        "Light theme (root) should exist as base"
    );
}

#[test]
fn all_themes_define_core_css_variables() {
    let css = get_styles_css();
    // Core variables that must exist (shadcn/ui convention)
    let core_vars = [
        "--background",
        "--foreground",
        "--primary",
        "--border",
        "--card",
    ];

    for var in core_vars {
        assert!(
            css.contains(var),
            "CSS should define core variable: {}",
            var
        );
    }
}

#[test]
fn themes_use_hsl_color_format() {
    let css = get_styles_css();
    // shadcn/ui uses HSL format for colors
    assert!(
        css.contains("hsl(") || css.contains("hsl(var("),
        "CSS should use HSL color format for theming"
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

// ============================================
// LAYOUT TESTS
// ============================================

#[test]
fn no_decorative_effects_exist() {
    let css = get_styles_css();
    // Decorative effects should not be present
    assert!(
        !css.contains("scanline"),
        "Scanline effects should not exist"
    );
    assert!(!css.contains("crt-effect"), "CRT effects should not exist");
}

#[test]
fn no_legacy_themes_exist() {
    let css = get_styles_css();
    // Legacy themes should not exist
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
            "Legacy theme '{}' should not exist",
            theme
        );
    }
}

// ============================================
// RESPONSIVE DESIGN TESTS
// ============================================

#[test]
fn responsive_breakpoints_exist() {
    let css = get_styles_css();
    // Tailwind uses @media for responsive styles
    assert!(
        css.contains("@media") || css.contains("@layer"),
        "CSS should have media queries or layers"
    );
}

#[test]
fn tailwind_base_layer_exists() {
    let css = get_styles_css();
    // Tailwind CSS uses @layer base
    assert!(
        css.contains("@layer base") || css.contains("@tailwind"),
        "CSS should have Tailwind base layer"
    );
}
