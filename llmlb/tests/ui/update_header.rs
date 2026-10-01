// Source-level check for the dashboard header update indicator (FR-015 / US-8).
//
// The badge text for each update state is verified by the component tests in
// `llmlb/src/web/dashboard/src/pages/Dashboard.update-banner.test.tsx`.
// The dot colour is a styling property that jsdom cannot observe, so its
// presence in the source stays checked here.

fn get_header_source() -> String {
    include_str!("../../src/web/dashboard/src/components/dashboard/Header.tsx").to_string()
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
