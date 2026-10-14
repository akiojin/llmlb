fn get_endpoint_table_source() -> String {
    super::source::dashboard_file("components/dashboard/EndpointTable.tsx")
}

#[test]
fn endpoint_table_does_not_render_tps_column() {
    let source = get_endpoint_table_source();
    let header = super::source::section(&source, "<TableHeader>", "</TableHeader>");
    assert!(
        !header.contains("TPS") && !source.contains("handleSort('tps')"),
        "EndpointTable should neither render nor sort an endpoint-level TPS column"
    );
    assert!(
        !source.contains("Aggregated endpoint TPS is hidden"),
        "EndpointTable should not include endpoint TPS placeholder guidance text"
    );
}
