alter table user_identities
    add column if not exists provider_subject text;

create unique index if not exists user_identities_provider_subject_idx
    on user_identities (provider_type, provider_subject)
    where provider_subject is not null;
