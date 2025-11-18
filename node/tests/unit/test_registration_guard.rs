use ollama_router_common::types::GpuDeviceInfo;
use or_node::registration::gpu_devices_valid;

#[test]
fn gpu_devices_validation_requires_entries() {
    let devices: Vec<GpuDeviceInfo> = Vec::new();
    assert!(
        !gpu_devices_valid(&devices),
        "GPU情報が空でも登録可能と判定されてはならない"
    );
}

#[test]
fn gpu_devices_validation_accepts_positive_counts() {
    let devices = vec![GpuDeviceInfo {
        model: "NVIDIA RTX 4090".to_string(),
        count: 2,
        memory: None,
    }];
    assert!(gpu_devices_valid(&devices));
}
