-- session（工作階段，data-model.md）。
--
-- 未採用 tower-sessions 泛用 Session Store：其以單一序列化 blob 儲存任意資料，
-- 不利於 FR-008/FR-009/FR-014 所需的「依 kind 雙逾時策略」與「即時撤銷」查詢／索引，
-- 故直接以本表明確欄位實作伺服器端可撤銷 Session（research.md R5 之決策不變，僅實作方式改為直接以 sqlx 操作本表）。
CREATE TABLE session (
    id              TEXT PRIMARY KEY,
    account_id      UUID NOT NULL REFERENCES account(id),
    kind            TEXT NOT NULL CHECK (kind IN ('web', 'desktop')),
    device_id       UUID NULL REFERENCES device(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_active_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL,
    revoked_at      TIMESTAMPTZ NULL
);
CREATE INDEX idx_session_account ON session(account_id);
