// Source-level check for the dashboard header update indicator (FR-015 / US-8).
//
// The badge text for each update state is verified by the component tests in
// `llmlb/src/web/dashboard/src/pages/Dashboard.update-banner.test.tsx`.
// The dot colour is a styling property that jsdom cannot observe, so its
// presence in the source stays checked here.

fn get_header_source() -> String {
    super::source::dashboard_file("components/dashboard/Header.tsx")
}

#[test]
fn header_shows_dot_indicator_for_update_state() {
    let source = get_header_source();
    // Connection status has the same colours. It must not rescue a broken
    // update indicator when its own dot is removed.
    let updating = super::source::section(
        &source,
        "{updateState && updateState !== 'up_to_date' && (",
        "{updateState === 'up_to_date' && (",
    );
    let current = super::source::section(&source, "{updateState === 'up_to_date' && (", "</p>");
    let updating = updating.split_whitespace().collect::<Vec<_>>().join(" ");
    // Dot colors: green for up_to_date, yellow for available/draining/applying, red for failed
    assert!(
        current.contains("bg-green-500"),
        "Header should show green dot for up_to_date state"
    );
    assert!(
        updating.contains(": 'bg-yellow-500'"),
        "Header should show yellow dot for available/updating states"
    );
    assert!(
        updating.contains("updateState === 'failed' ? 'bg-red-500'"),
        "Header should show red dot for failed state"
    );
}
