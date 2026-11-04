//! Source-level lock-order assertions. Real lock contention is qualified by the
//! marker-owned PostgreSQL test.
use std::fs;
use std::path::Path;

fn source(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

#[test]
fn each_personal_writer_acquires_the_shared_user_lock_before_fence_and_event() {
    let repository = fs::read_to_string(source("src/adapters/out/repository.rs")).unwrap();
    let avatar = fs::read_to_string(source("src/adapters/out/repository/avatar.rs")).unwrap();
    let coordinator = fs::read_to_string(source("src/participant/live/coordinator.rs")).unwrap();
    for source in [repository, avatar, coordinator] {
        let lock = source
            .find("account_deletion::lock_user_in")
            .expect("user lock");
        let fence = source
            .find("account_deletion::is_fenced_in")
            .expect("fence check");
        assert!(lock < fence, "fence must be observed after the user lock");
    }
}

#[test]
fn migration_blocks_direct_recreation_after_the_permanent_fence() {
    let sql = fs::read_to_string(source("migrations/0055_private_data_deletion.sql")).unwrap();
    assert!(sql.contains("concert_private_data_participant_write_guard"));
    assert!(sql.contains("concert_private_data_avatar_write_guard"));
    assert!(sql.contains("BEFORE INSERT OR UPDATE OR DELETE ON public.concert_participants"));
    assert!(sql.contains("BEFORE INSERT OR UPDATE OR DELETE ON public.concert_avatar_settings"));
    assert!(sql.contains("mcr.private_data_deletion_user"));
}
