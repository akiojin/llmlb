// Contract tests for HF button removal (FR-028)
// Models section should not have Hugging Face related buttons

// Read the index.html file content for testing
fn get_index_html() -> String {
    include_str!("../../src/web/static/index.html").to_string()
}

#[test]
fn models_section_has_no_hf_search() {
    let html = get_index_html();
    assert!(
        !html.contains("id=\"hf-search\""),
        "HF search input should be removed"
    );
    assert!(
        !html.contains("Search HF"),
        "Search HF label should be removed"
    );
}

#[test]
fn models_section_has_no_hf_refresh() {
    let html = get_index_html();
    assert!(
        !html.contains("id=\"hf-refresh\""),
        "HF refresh button should be removed"
    );
    assert!(
        !html.contains("Refresh HF"),
        "Refresh HF text should be removed"
    );
}

#[test]
fn models_section_has_no_registered_refresh() {
    let html = get_index_html();
    assert!(
        !html.contains("id=\"registered-refresh\""),
        "Registered refresh button should be removed"
    );
    assert!(
        !html.contains("Refresh Registered"),
        "Refresh Registered text should be removed"
    );
}

#[test]
fn models_section_has_no_download_tasks_refresh() {
    let html = get_index_html();
    assert!(
        !html.contains("id=\"download-tasks-refresh\""),
        "Download tasks refresh button should be removed"
    );
    assert!(
        !html.contains("Refresh Tasks"),
        "Refresh Tasks text should be removed"
    );
}

#[test]
fn models_section_has_no_section_tools() {
    let html = get_index_html();
    // The entire section-tools container should be removed from models section
    // Check that section-tools doesn't appear in the models tab context
    let models_section = html.find("id=\"tab-models\"").map(|start| {
        let end = html[start..].find("</div>").unwrap_or(html.len() - start);
        &html[start..start + end + 200] // Get enough context
    });

    if let Some(section) = models_section {
        assert!(
            !section.contains("section-tools"),
            "Models section should not have section-tools container"
        );
    }
}

#[test]
fn dashboard_has_no_japanese_text() {
    let html = get_index_html();
    assert!(
        !html.contains("対応可能モデル"),
        "Japanese text should be removed from models section"
    );
    assert!(
        !html.contains("対応モデル"),
        "Japanese text should be removed from models section"
    );
    assert!(
        !html.contains("テスト用コンソール"),
        "Japanese text should be removed from chat modal"
    );
}
