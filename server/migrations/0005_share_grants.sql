-- share_grant（分享授權，data-model.md）
CREATE TABLE share_grant (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_account_id      UUID NOT NULL REFERENCES account(id),
    grantee_account_id    UUID NOT NULL REFERENCES account(id),
    scope                 TEXT NOT NULL,
    revoked_at            TIMESTAMPTZ NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (owner_account_id <> grantee_account_id)
);
CREATE INDEX idx_share_grant_grantee ON share_grant(grantee_account_id);
CREATE INDEX idx_share_grant_owner ON share_grant(owner_account_id);
