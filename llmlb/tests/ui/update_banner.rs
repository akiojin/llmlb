// Regression tests for dashboard update banner behavior (FR-006/FR-007)
//
// This verifies source-level guarantees for the update banner:
// - The "Check for updates" CTA remains present.
// - The banner no longer disappears when update state is temporarily unavailable.

fn get_dashboard_source() -> String {
    include_str!("../../src/web/dashboard/src/pages/Dashboard.tsx").to_string()
}

#[test]
fn check_for_updates_button_is_present() {
    let source = get_dashboard_source();
    assert!(
        source.contains("Check for updates"),
        "Dashboard should expose the manual update-check CTA"
    );
}

#[test]
fn update_banner_does_not_early_return_when_update_missing() {
    let source = get_dashboard_source();
    assert!(
        !source.contains("if (!update) return null"),
        "Update banner should stay visible even when update state is temporarily unavailable"
    );
    assert!(
        source.contains("Update status unavailable"),
        "Dashboard should show a fallback description while update state is unavailable"
    );
}

#[test]
fn restart_button_visibility_depends_on_update_state() {
    let source = get_dashboard_source();
    assert!(
        source.contains("updateState === 'available' || updateState === 'failed' || applying"),
        "Restart button visibility should be tied to explicit update states"
    );
}
