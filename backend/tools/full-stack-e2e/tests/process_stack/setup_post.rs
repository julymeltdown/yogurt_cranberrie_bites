//! A timed-out setup command is not rollback. Resolve the single original
//! operation before using its fixture row; never submit another creation.
use super::{process_json_request, process_json_request_with_operation_id, ProcessUser};
use anyhow::{ensure, Context, Result};
use post_service::application::dto::mutation_dto::MutationOutcomeDto;
use reqwest::{Method, StatusCode};
use serde_json::json;
use sqlx::{Executor, PgPool};
use std::time::Duration;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

pub async fn create(
    client: &reqwest::Client,
    base: &str,
    gateway: &PgPool,
    post: &PgPool,
    author: &ProcessUser,
    content: &str,
) -> Result<Uuid> {
    // Pause only completion bookkeeping for this actor's setup POST. The
    // owner must commit while the unchanged 300 ms caller budget expires.
    let function = format!("CREATE FUNCTION e2e_setup_post_completion() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.actor_id = '{}' AND NEW.action = 'POST /api/posts' THEN PERFORM pg_advisory_xact_lock(19413, 1); END IF; RETURN NEW; END; $$", author.user_id);
    gateway.execute(function.as_str()).await?;
    gateway.execute("create trigger e2e_setup_post_completion before insert on admin_audit_events for each row execute function e2e_setup_post_completion()").await?;
    let mut barrier = gateway.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(19413, 1)")
        .execute(&mut *barrier)
        .await?;
    let response = process_json_request_with_operation_id(
        client,
        Method::POST,
        base,
        "/api/posts",
        Some(&author.access_token),
        None,
        Some(json!({"content": content, "visibility": "PUBLIC"})),
    )
    .await;
    // Release before polling or returning any request error. The transaction
    // also releases on drop if this fixture unwinds.
    barrier.rollback().await?;
    let result = async {
        let (status, _, operation_id) = response?;
        ensure!(status == StatusCode::GATEWAY_TIMEOUT, "setup completion barrier did not force the original timeout: {status}");
        let operation_id = operation_id.context("timed-out setup command omitted operation identity")?;
        timeout(Duration::from_secs(10), async {
            loop {
                let (status, operation) = process_json_request(
                    client, Method::GET, base, &format!("/api/operations/{operation_id}"),
                    Some(&author.access_token), None, None,
                ).await?;
                ensure!(status == StatusCode::OK, "setup operation lookup failed: {status}");
                ensure!(operation["operationId"] == operation_id.to_string(), "setup lookup returned another operation");
                ensure!(operation["outcome"] != "REJECTED_BEFORE_COMMIT", "setup command was durably rejected");
                if operation["state"] == "COMPLETED" {
                    ensure!(operation["outcome"] == "COMMITTED" && operation["responseStatus"] == 200, "setup command did not complete successfully");
                    break Ok::<(), anyhow::Error>(());
                }
                sleep(Duration::from_millis(50)).await;
            }
        }).await.context("original setup operation did not settle")??;
        // Fixture-only owner observation: the public status endpoint correctly
        // excludes the saved response body. Bind the row to the exact original
        // operation and actor, instead of guessing an ID or retrying the POST.
        let value: serde_json::Value = sqlx::query_scalar("select outcome_json from post_mutation_receipts where operation_id=$1 and actor_id=$2 and scope='post.v1.CreatePost'")
            .bind(operation_id).bind(author.user_id).fetch_one(post).await?;
        let MutationOutcomeDto::Post(created, _) = serde_json::from_value(value)? else {
            anyhow::bail!("setup owner receipt is not a created post");
        };
        ensure!(created.author_id == author.user_id && created.content == content, "setup receipt belongs to another command");
        let rows: Vec<Uuid> = sqlx::query_scalar("select id from posts where author_id=$1 and content=$2")
            .bind(author.user_id).bind(content).fetch_all(post).await?;
        ensure!(rows == [created.id], "setup command did not produce exactly its one receipted post");
        eprintln!("SETUP_POST_TIMEOUT_RECOVERED operation_id={operation_id} post_id={} effects=1", created.id);
        Ok(created.id)
    }.await;
    gateway.execute("drop trigger e2e_setup_post_completion on admin_audit_events; drop function e2e_setup_post_completion()").await?;
    result
}
