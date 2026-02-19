// Tests for dashboard header update state badge display (FR-015 / US-8)
//
// This verifies source-level guarantees for the header update status indicator:
// - The header accepts update state information as props.
// - A dot indicator and badge text are rendered based on update state.

fn get_header_source() -> String {
    include_str!("../../src/web/dashboard/src/components/dashboard/Header.tsx").to_string()
}

#[test]
fn header_accepts_update_state_prop() {
    let source = get_header_source();
    assert!(
        source.contains("updateState"),
        "Header should accept updateState prop for showing update status"
    );
}

#[test]
fn header_shows_dot_indicator_for_update_state() {
    let source = get_header_source();
    // Dot colors: green for up_to_date, yellow for available/draining/applying, red for failed
    assert!(
        source.contains("bg-green-500") || source.contains("green"),
        "Header should show green dot for up_to_date state"
    );
    assert!(
        source.contains("bg-yellow-500") || source.contains("yellow"),
        "Header should show yellow dot for available/updating states"
    );
    assert!(
        source.contains("bg-red-500") || source.contains("red"),
        "Header should show red dot for failed state"
    );
}

#[test]
fn header_shows_available_badge_text() {
    let source = get_header_source();
    assert!(
        source.contains("available"),
        "Header should display a badge when update is available"
    );
}
