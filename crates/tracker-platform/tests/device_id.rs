//! 單元測試：裝置指紋讀取（trait 抽象＋假硬體來源，T022）。

use tracker_platform::device_id::{DeviceIdSource, FakeDeviceIdSource};

#[test]
fn returns_configured_guid() {
    let source = FakeDeviceIdSource::new("11111111-2222-3333-4444-555555555555");
    assert_eq!(source.machine_guid().as_deref(), Some("11111111-2222-3333-4444-555555555555"));
}

#[test]
fn returns_none_when_unavailable() {
    let source = FakeDeviceIdSource::missing();
    assert_eq!(source.machine_guid(), None);
}
