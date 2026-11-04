-- Installed by the Post root migration owner; never implicit runtime DDL.
-- References, owner IDs, binding bytes and receipts are not diagnostics.
CREATE TABLE post_concert_read_tracking (
    settlement_reference bytea PRIMARY KEY CHECK (octet_length(settlement_reference)=32),
    producer_instance_id uuid NOT NULL,
    transaction_bytes bytea NOT NULL CHECK (octet_length(transaction_bytes) BETWEEN 1 AND 4096),
    binding_bytes bytea NOT NULL CHECK (octet_length(binding_bytes) BETWEEN 1 AND 65536),
    binding_digest bytea NOT NULL CHECK (octet_length(binding_digest)=32),
    auth_guard_id uuid NOT NULL,
    friend_guard_id uuid NOT NULL,
    expires_at timestamptz NOT NULL,
    auth_attempted boolean NOT NULL DEFAULT false,
    friend_attempted boolean NOT NULL DEFAULT false,
    auth_registration_id uuid,
    friend_registration_id uuid,
    witness bytea NOT NULL CHECK (octet_length(witness)=32),
    terminal_status text CHECK (terminal_status IN ('COMMITTED','ABORTED')),
    CHECK (auth_registration_id IS NULL OR auth_attempted),
    CHECK (friend_registration_id IS NULL OR friend_attempted)
);
CREATE INDEX post_concert_read_tracking_pending
    ON post_concert_read_tracking(producer_instance_id) WHERE terminal_status IS NULL;
CREATE TABLE post_concert_read_terminal_witness (
    settlement_reference bytea PRIMARY KEY REFERENCES post_concert_read_tracking(settlement_reference),
    transaction_bytes bytea NOT NULL,
    binding_digest bytea NOT NULL CHECK (octet_length(binding_digest)=32),
    witness bytea NOT NULL CHECK (octet_length(witness)=32)
);
