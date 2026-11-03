-- BE26 isolated schema artifact v1; NOT a registered runtime migration.
create table concert_age_owners(user_id uuid primary key check(user_id<>'00000000-0000-0000-0000-000000000000'));
create table concert_age_verifications(
 verification_id uuid primary key check(verification_id<>'00000000-0000-0000-0000-000000000000'), user_id uuid not null references concert_age_owners,
 event_id uuid not null references concert_events on delete restrict, session_id uuid not null check(session_id<>'00000000-0000-0000-0000-000000000000'),
 verifier_id text not null check(verifier_id ~ '^[A-Za-z0-9._-]{1,100}$'),
 source_revision text not null check(source_revision ~ '^[A-Za-z0-9._-]{1,100}$'),
 state_digest bytea check(state_digest is null or octet_length(state_digest)=32),
 return_target text not null check(return_target='AGE_PRIVACY'),
 created_at timestamptz not null, expires_at timestamptz not null,
 state text not null check(state in ('PENDING','RESERVED','EXCHANGING','DONE','EXPIRED')),
 attempt_id uuid, callback_key uuid, receipt_id uuid references concert_command_receipts on delete restrict,
 unique(verification_id,user_id,event_id,verifier_id,source_revision),
 unique(receipt_id),
 check(expires_at>created_at and expires_at<=created_at+interval '10 minutes'),
 check((state='PENDING' and attempt_id is null and callback_key is null and receipt_id is null and state_digest is not null)
   or (state in ('RESERVED','EXCHANGING','DONE') and attempt_id is not null and callback_key is not null and receipt_id is not null)
   or state='EXPIRED')
);
create index concert_age_state_expiry on concert_age_verifications(user_id,expires_at,verification_id) where state<>'EXPIRED';
create table concert_age_claims(
 claim_id uuid primary key references concert_age_verifications on delete restrict,
 user_id uuid not null references concert_age_owners, event_id uuid not null references concert_events on delete restrict,
 verifier_id text not null check(verifier_id ~ '^[A-Za-z0-9._-]{1,100}$'),
 source_revision text not null check(source_revision ~ '^[A-Za-z0-9._-]{1,100}$'),
 status text not null check(status in ('VERIFIED','REPLACED','REVOKED','UNSUPPORTED','EXPIRED')),
 verified_at timestamptz not null, expires_at timestamptz not null,
 age_at_verification smallint, next_birthday_at date,
 foreign key(claim_id,user_id,event_id,verifier_id,source_revision) references concert_age_verifications(verification_id,user_id,event_id,verifier_id,source_revision) on delete restrict,
 check(expires_at>verified_at and expires_at<=verified_at+interval '90 days'),
 check((status='UNSUPPORTED' and age_at_verification is null and next_birthday_at is null)
  or (status='EXPIRED' and expires_at<=clock_timestamp() and age_at_verification is null and next_birthday_at is null)
  or (status in ('REPLACED','REVOKED') and expires_at<=clock_timestamp() and age_at_verification is null and next_birthday_at is null)
  or (status in ('VERIFIED','REPLACED','REVOKED') and age_at_verification is not null and next_birthday_at is not null and age_at_verification between 14 and 100
   and next_birthday_at>(verified_at at time zone 'Asia/Seoul')::date
   and next_birthday_at<=(verified_at at time zone 'Asia/Seoul')::date+366))
);
create index concert_age_canonical on concert_age_claims(user_id,verified_at desc,expires_at desc,claim_id desc);

create unique index concert_age_one_verified on concert_age_claims(user_id) where status='VERIFIED';

create index concert_age_private_expiry on concert_age_claims(user_id,expires_at,claim_id) where age_at_verification is not null;
