/// Contract tests for theme visual effects (FR-027)
/// Each theme should have unique visual effects beyond just color changes
///
/// NEW SKIN SYSTEM: 3 distinct layout-based themes
/// - Minimal: Single-column centered layout, flat design
/// - Tech: Sidebar layout with terminal aesthetic
/// - Creative: Asymmetric magazine-style layout

/// Read the styles.css file content for testing
fn get_styles_css() -> String {
    include_str!("../../src/web/static/styles.css").to_string()
}

// ============================================
// NEW SKIN SYSTEM TESTS (3 Themes)
// ============================================

#[test]
fn minimal_theme_has_single_column_stats_layout() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"minimal\"] .stats-grid"),
        "Minimal theme should have stats-grid styles"
    );
    // Single column layout uses flex-direction: column
    assert!(
        css.contains("flex-direction: column"),
        "Minimal theme should use single-column flex layout"
    );
}

#[test]
fn minimal_theme_has_flat_card_design() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"minimal\"] .stat-card"),
        "Minimal theme should have stat-card styles"
    );
}

#[test]
fn minimal_theme_hides_grid_overlay() {
    let css = get_styles_css();
    // Minimal theme should hide the grid/scanline effects
    assert!(
        css.contains("[data-theme=\"minimal\"] body::before"),
        "Minimal theme should override body::before (grid overlay)"
    );
}

#[test]
fn tech_theme_has_sidebar_stats_layout() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"tech\"] .stats-grid"),
        "Tech theme should have stats-grid styles"
    );
    // Sidebar uses position: fixed
    assert!(
        css.contains("position: fixed"),
        "Tech theme should use fixed positioning for sidebar"
    );
}

#[test]
fn tech_theme_has_monospace_font() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"tech\"]"),
        "Tech theme selector should exist"
    );
    // Tech theme uses monospace font
    assert!(
        css.contains("monospace"),
        "Tech theme should use monospace font"
    );
}

#[test]
fn tech_theme_has_terminal_card_style() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"tech\"] .stat-card"),
        "Tech theme should have stat-card styles"
    );
}

#[test]
fn creative_theme_has_asymmetric_grid() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"creative\"] .stats-grid"),
        "Creative theme should have stats-grid styles"
    );
}

#[test]
fn creative_theme_has_rotated_cards() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"creative\"] .stat-card"),
        "Creative theme should have stat-card styles"
    );
    // Creative theme uses rotation
    assert!(
        css.contains("rotate"),
        "Creative theme should use card rotation"
    );
}

#[test]
fn creative_theme_has_vertical_header() {
    let css = get_styles_css();
    // Creative theme has vertical writing mode for header
    assert!(
        css.contains("[data-theme=\"creative\"] .page-header"),
        "Creative theme should have page-header styles"
    );
}

#[test]
fn all_three_themes_have_distinct_layouts() {
    let css = get_styles_css();
    // All 3 themes must define their own stats-grid layout
    assert!(
        css.contains("[data-theme=\"minimal\"] .stats-grid"),
        "Minimal theme layout must exist"
    );
    assert!(
        css.contains("[data-theme=\"tech\"] .stats-grid"),
        "Tech theme layout must exist"
    );
    assert!(
        css.contains("[data-theme=\"creative\"] .stats-grid"),
        "Creative theme layout must exist"
    );
}

// ============================================
// LEGACY TESTS (7 Themes - to be removed after migration)
// ============================================

#[test]
fn synthwave_theme_has_pulse_animation() {
    let css = get_styles_css();
    assert!(
        css.contains("synthwave-pulse"),
        "Synthwave theme should have pulse animation"
    );
    assert!(
        css.contains("[data-theme=\"synthwave\"]"),
        "Synthwave theme selector should exist"
    );
}

#[test]
fn ocean_theme_has_wave_animation() {
    let css = get_styles_css();
    assert!(
        css.contains("ocean-wave"),
        "Ocean theme should have wave animation"
    );
    assert!(
        css.contains("[data-theme=\"ocean\"]"),
        "Ocean theme selector should exist"
    );
}

#[test]
fn ember_theme_has_flicker_animation() {
    let css = get_styles_css();
    assert!(
        css.contains("ember-flicker"),
        "Ember theme should have flicker animation"
    );
    assert!(
        css.contains("[data-theme=\"ember\"]"),
        "Ember theme selector should exist"
    );
}

#[test]
fn forest_theme_has_sway_animation() {
    let css = get_styles_css();
    assert!(
        css.contains("forest-sway"),
        "Forest theme should have sway animation"
    );
    assert!(
        css.contains("[data-theme=\"forest\"]"),
        "Forest theme selector should exist"
    );
}

#[test]
fn mono_theme_exists_with_minimal_effects() {
    let css = get_styles_css();
    assert!(
        css.contains("[data-theme=\"mono\"]"),
        "Mono theme selector should exist"
    );
    // Mono should have minimal effects (no scanlines)
    assert!(
        css.contains("[data-theme=\"mono\"]") && css.contains("--scanline-opacity: 0"),
        "Mono theme should have scanline-opacity: 0"
    );
}

#[test]
fn prefers_reduced_motion_disables_animations() {
    let css = get_styles_css();
    assert!(
        css.contains("prefers-reduced-motion"),
        "CSS should respect prefers-reduced-motion for accessibility"
    );
}

#[test]
fn theme_animations_target_main_not_dashboard_container() {
    let css = get_styles_css();
    // .dashboard-container should not exist - animations should target main element
    assert!(
        !css.contains(".dashboard-container"),
        "CSS should use main element instead of .dashboard-container"
    );
    // Verify main element has animations applied
    assert!(
        css.contains("[data-theme=\"retro\"] main"),
        "Retro theme should animate main element"
    );
    assert!(
        css.contains("[data-theme=\"synthwave\"] main::after"),
        "Synthwave theme should use main::after for horizon glow"
    );
    assert!(
        css.contains("[data-theme=\"ember\"] main"),
        "Ember theme should animate main element"
    );
}
