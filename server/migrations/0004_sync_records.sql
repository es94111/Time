-- sync_record（同步紀錄，data-model.md）
CREATE TABLE sync_record (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    device_id                 UUID NOT NULL REFERENCES device(id),
    batch_dedup_key           TEXT NOT NULL UNIQUE,
    device_local_time_range   TSTZRANGE NOT NULL,
    server_received_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    status                    TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'completed', 'failed')),
    object_storage_key        TEXT NULL,
    record_count              INT NOT NULL DEFAULT 0
);
CREATE INDEX idx_sync_record_device_time ON sync_record(device_id, server_received_at);
