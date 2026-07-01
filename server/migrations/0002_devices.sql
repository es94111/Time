-- device（裝置，data-model.md）
CREATE TABLE device (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id            UUID NOT NULL REFERENCES account(id),
    hardware_fingerprint  TEXT NOT NULL,
    display_name          TEXT NOT NULL,
    last_sync_at          TIMESTAMPTZ NULL,
    removed_at            TIMESTAMPTZ NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_device_account ON device(account_id);
-- 僅限「使用中」裝置的 (account_id, hardware_fingerprint) 唯一：
-- 移除後重新登入視為全新裝置，故唯一鍵僅套用於 removed_at IS NULL 的列。
CREATE UNIQUE INDEX idx_device_active_fingerprint
    ON device(account_id, hardware_fingerprint)
    WHERE removed_at IS NULL;
