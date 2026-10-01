fn get_endpoint_table_source() -> String {
    include_str!("../../src/web/dashboard/src/components/dashboard/EndpointTable.tsx").to_string()
}

#[test]
fn endpoint_table_does_not_render_tps_column() {
    let source = get_endpoint_table_source();
    assert!(
        !source.contains("handleSort('tps')"),
        "EndpointTable should not sort by TPS because endpoint-level TPS is not shown"
    );
    assert!(
        !source.contains("Aggregated endpoint TPS is hidden"),
        "EndpointTable should not include endpoint TPS placeholder guidance text"
    );
}
