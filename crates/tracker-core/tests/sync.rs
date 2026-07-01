//! 單元測試：批次切分／去重鍵純邏輯（T023）。

use tracker_core::sync::{make_dedup_key, split_into_batches, BatchLimits, QueueItem, UploadState};

fn item(id: i64, bytes: usize, ts: i64) -> QueueItem {
    QueueItem { id, payload_bytes: bytes, created_at_local_ms: ts }
}

#[test]
fn splits_by_record_count_limit() {
    let limits = BatchLimits { max_records: 2, max_bytes: usize::MAX };
    let items = vec![item(1, 10, 0), item(2, 10, 1), item(3, 10, 2), item(4, 10, 3), item(5, 10, 4)];
    let batches = split_into_batches(&items, &limits);
    assert_eq!(batches, vec![vec![1, 2], vec![3, 4], vec![5]]);
}

#[test]
fn splits_by_byte_size_limit() {
    let limits = BatchLimits { max_records: usize::MAX, max_bytes: 25 };
    let items = vec![item(1, 10, 0), item(2, 10, 1), item(3, 10, 2)];
    let batches = split_into_batches(&items, &limits);
    // 10+10=20 <= 25，加入第三筆會變 30 > 25，故另起一批。
    assert_eq!(batches, vec![vec![1, 2], vec![3]]);
}

#[test]
fn oversized_single_item_forms_its_own_batch() {
    let limits = BatchLimits { max_records: 10, max_bytes: 5 };
    let items = vec![item(1, 100, 0)];
    let batches = split_into_batches(&items, &limits);
    assert_eq!(batches, vec![vec![1]]);
}

#[test]
fn empty_input_yields_no_batches() {
    let batches = split_into_batches(&[], &BatchLimits::default());
    assert!(batches.is_empty());
}

#[test]
fn dedup_key_is_deterministic_and_unique_per_seq() {
    let a = make_dedup_key("device-abc", 1);
    let b = make_dedup_key("device-abc", 2);
    let c = make_dedup_key("device-abc", 1);
    assert_ne!(a, b);
    assert_eq!(a, c);
}

#[test]
fn upload_state_round_trips_through_string() {
    for state in [
        UploadState::Queued,
        UploadState::Uploading,
        UploadState::Uploaded,
        UploadState::DiscardedDiskFull,
    ] {
        assert_eq!(UploadState::parse(state.as_str()), Some(state));
    }
    assert_eq!(UploadState::parse("unknown"), None);
}
