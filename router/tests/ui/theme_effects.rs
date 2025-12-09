/// Contract tests for theme visual effects (FR-027)
/// Each theme should have unique visual effects beyond just color changes

/// Read the styles.css file content for testing
fn get_styles_css() -> String {
    include_str!("../../src/web/static/styles.css").to_string()
}

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
