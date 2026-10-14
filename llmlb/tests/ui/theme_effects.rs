// Contract tests for theme system (FR-027)
// React + Tailwind + shadcn/ui based theme system: Dark, Light
//
// Design principles:
// - CSS custom properties for theme colors via Tailwind
// - Dark/Light mode toggle via class on html element
// - Accessibility support (prefers-reduced-motion)

/// Read the index.css file content for testing (React dashboard source)
fn get_styles_css() -> String {
    super::source::dashboard_file("index.css")
}

fn theme_variables<'a>(css: &'a str, selector: &str) -> &'a str {
    // Match the whole selector, not the suffix of e.g. `.unused .dark`.
    let rule = regex::Regex::new(&format!(
        r"(?m)^[ \t]*{}[ \t]*\{{([^}}]*)\}}",
        regex::escape(selector)
    ))
    .expect("theme rule expression must be valid");
    rule.captures(css)
        .and_then(|captures| captures.get(1))
        .unwrap_or_else(|| panic!("theme rule missing: {selector}"))
        .as_str()
}

// ============================================
// THEME SYSTEM TESTS (Dark/Light Themes)
// ============================================

#[test]
fn dark_theme_exists() {
    let css = get_styles_css();
    // Theme selection belongs to useTheme; this contract checks the dark palette.
    assert!(
        theme_variables(&css, ".dark").contains("--background:"),
        ".dark should define theme variables"
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

    for selector in [":root", ".dark"] {
        let variables = theme_variables(&css, selector);
        for var in core_vars {
            assert!(
                variables.contains(&format!("{var}:")),
                "{selector} should define core variable: {var}"
            );
        }
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
    // Tailwind v4 generates viewport rules from responsive classes in Views.
    // The stylesheet's reduced-motion @media is not a viewport breakpoint.
    let header = super::source::dashboard_file("components/dashboard/Header.tsx");
    assert!(
        header.contains("sm:") || header.contains("md:") || header.contains("lg:"),
        "Header should use responsive Tailwind breakpoints"
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
