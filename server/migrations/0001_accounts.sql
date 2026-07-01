-- account（使用者帳號，data-model.md）
CREATE TABLE account (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email_or_username   TEXT NOT NULL UNIQUE,
    password_hash       TEXT NOT NULL,
    failed_login_count  INT NOT NULL DEFAULT 0,
    locked_until        TIMESTAMPTZ NULL,
    retention_days      INT NULL,
    deleted_at          TIMESTAMPTZ NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
