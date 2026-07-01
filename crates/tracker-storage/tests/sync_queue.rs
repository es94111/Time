//! 整合測試：磁碟滿捨棄最舊、`dedup_key` 唯一鍵（T035）。

use std::path::PathBuf;

use tracker_storage::sync_queue_repo::SyncQueueRepo;
use tracker_storage::Database;

fn unique_temp_dir() -> PathBuf {
    let mut d = std::env::temp_dir();
    let nanos =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    d.push(format!("tracker_sync_queue_test_{nanos}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn open_db() -> Database {
    let dir = unique_temp_dir();
    Database::open(&dir.join("test.db"), &[7u8; 32]).unwrap()
}

#[test]
fn dedup_key_is_unique_and_reinsert_is_ignored() {
    let db = open_db();
    let repo = SyncQueueRepo::new(&db);

    repo.enqueue(b"payload-a", "2026-07-01T00:00:00Z", "dedup-1").unwrap();
    // 重複插入相同 dedup_key 應被忽略（INSERT OR IGNORE），不視為錯誤、不重複計數。
    repo.enqueue(b"payload-a-retry", "2026-07-01T00:00:01Z", "dedup-1").unwrap();

    assert_eq!(repo.queued_count().unwrap(), 1);
    let items = repo.queued_items(10).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].payload, b"payload-a");
}

#[test]
fn oldest_queued_items_are_discarded_when_over_limit() {
    let db = open_db();
    let repo = SyncQueueRepo::new(&db);

    for i in 0..5 {
        let ts = format!("2026-07-01T00:00:{i:02}Z");
        let key = format!("dedup-{i}");
        repo.enqueue(&vec![0u8; 100], &ts, &key).unwrap();
    }
    assert_eq!(repo.pending_bytes().unwrap(), 500);

    // 上限僅容納 3 筆（300 bytes），應由舊到新捨棄前兩筆。
    let discarded = repo.discard_oldest_if_over(300).unwrap();
    assert!(discarded);
    assert_eq!(repo.pending_bytes().unwrap(), 300);
    assert_eq!(repo.queued_count().unwrap(), 3);

    let items = repo.queued_items(10).unwrap();
    let remaining_keys: Vec<_> = items.iter().map(|r| r.dedup_key.clone()).collect();
    assert_eq!(remaining_keys, vec!["dedup-2", "dedup-3", "dedup-4"]);
}

#[test]
fn no_discard_when_within_limit() {
    let db = open_db();
    let repo = SyncQueueRepo::new(&db);
    repo.enqueue(&vec![0u8; 100], "2026-07-01T00:00:00Z", "dedup-only").unwrap();

    let discarded = repo.discard_oldest_if_over(1_000_000).unwrap();
    assert!(!discarded);
    assert_eq!(repo.queued_count().unwrap(), 1);
}
