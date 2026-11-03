use anyhow::{ensure, Result};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Erase capture state in the existing subject-fenced cleanup transaction.
/// The presence check keeps pre-0023 schema upgrade probes executable.
pub(super) async fn forget_affinity_sources(
    tx: &mut Transaction<'_, Postgres>,
    subject: Uuid,
) -> Result<()> {
    let installed: bool = sqlx::query_scalar(
        "select to_regprocedure('feed_affinity_forget_subject(uuid)') is not null",
    )
    .fetch_one(&mut **tx)
    .await?;
    if installed {
        sqlx::query("select feed_affinity_forget_subject($1)")
            .bind(subject)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// All post writers acquire the complete sorted subject set before the post
/// row. The owner is immutable through supported operations; fail on a race.
pub(super) async fn lock_post_subjects(
    tx: &mut Transaction<'_, Postgres>,
    post_id: Uuid,
    extra: &[Uuid],
) -> Result<()> {
    lock_posts_subjects(tx, &[post_id], extra).await
}

/// Batch writers must acquire the complete subject set before ANY post row;
/// incrementally locking one post at a time can invert subject ordering.
pub(super) async fn lock_posts_subjects(
    tx: &mut Transaction<'_, Postgres>,
    post_ids: &[Uuid],
    extra: &[Uuid],
) -> Result<()> {
    let observed: Vec<(Uuid, Uuid)> =
        sqlx::query_as("select id,author_id from posts where id=any($1) order by id")
            .bind(post_ids)
            .fetch_all(&mut **tx)
            .await?;
    let mut subjects = extra.to_vec();
    subjects.extend(observed.iter().map(|(_, author)| *author));
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(subjects)
        .execute(&mut **tx)
        .await?;
    let locked: Vec<(Uuid, Uuid)> =
        sqlx::query_as("select id,author_id from posts where id=any($1) order by id for update")
            .bind(post_ids)
            .fetch_all(&mut **tx)
            .await?;
    ensure!(
        observed == locked,
        "Post owner changed during subject locking"
    );
    Ok(())
}
