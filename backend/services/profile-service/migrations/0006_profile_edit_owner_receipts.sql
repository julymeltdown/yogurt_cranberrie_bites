-- Profile edits share the privacy receipt table and retention policy, but bind
-- to their own versioned RPC scope so a key cannot replay across commands.
alter table profile_mutation_receipts
    drop constraint if exists profile_mutation_receipts_scope_check;

alter table profile_mutation_receipts
    add constraint profile_mutation_receipts_scope_check
        check (scope in (
            'profile.v1.UpdateProfilePrivacy',
            'profile.v1.UpdateProfile'
        ));
