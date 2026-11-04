//! Actual correlation SQL only. Direct intent installation is NOT canonical auth.
use super::*;
use crate::gift_live::{protocol::*, queries::native_tests::Fixture};
use chrono::{Duration, Utc};
use serde_json::{json, Value};

fn request(
    f: &Fixture,
    op: GiftOperation,
    root: Uuid,
    reservation: Uuid,
    key: Uuid,
) -> GiftRequest {
    let fields = json!({"title":"Gift correlation fixture","description":"","image_media_id":null,
        "quantity_total":3,"pickup_starts_at":"2035-11-07T08:00:00Z","pickup_ends_at":"2035-11-07T09:00:00Z","public_place_id":Uuid::new_v4()});
    let (actor, method, path, body) = match op {
        GiftOperation::Create => (f.owner, "POST", "giveaways".into(), Some(fields)),
        GiftOperation::Update => {
            let mut body = fields;
            body["expected_revision"] = json!(1);
            body["state"] = json!("OPEN");
            (f.owner, "PATCH", format!("giveaways/{root}"), Some(body))
        }
        GiftOperation::SetMeeting => (
            f.owner,
            "PUT",
            format!("giveaways/{root}/pickup-meeting"),
            Some(
                json!({"expected_revision":1,"physical_presence_acknowledged":true,
            "meeting":{"place_id":Uuid::new_v4(),"map_revision":1,"place_revision":1,"instructions":"Synthetic","identification":"Test only","expires_at":"2035-11-07T09:00:00Z"}}),
            ),
        ),
        GiftOperation::Reserve => (
            f.recipients[0],
            "POST",
            format!("giveaways/{root}/reservations"),
            Some(json!({"giveaway_revision":1})),
        ),
        GiftOperation::Cancel => (
            f.recipients[0],
            "DELETE",
            format!("reservations/{reservation}?expected_revision=1"),
            None,
        ),
        GiftOperation::IssueCode => (
            f.recipients[0],
            "POST",
            format!("reservations/{reservation}/pickup-code"),
            Some(json!({"expected_revision":1})),
        ),
        GiftOperation::Claim => (
            f.owner,
            "POST",
            format!("reservations/{reservation}/claim"),
            Some(json!({"expected_revision":1,"confirmation_code":"012345"})),
        ),
        _ => unreachable!(),
    };
    GiftRequest::from_verified(
        f.identity(actor),
        op.id(),
        method,
        &format!("/api/concert/events/{}/{path}", f.event),
        if body.is_some() {
            "application/json"
        } else {
            ""
        },
        &key.to_string(),
        body.map(|v| serde_json::to_vec(&v).unwrap())
            .unwrap_or_default(),
    )
    .ok()
    .unwrap()
}
fn error(e: CommandError) -> anyhow::Error {
    anyhow::anyhow!("{e:?}")
}

#[tokio::test]
#[ignore = "private Gift PG fixture; local correlation SQL only"]
async fn every_live_mutation_installs_exact_target_only_materialized_with_receipt(
) -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let root = f.gift(Utc::now() - Duration::minutes(1)).await?;
    let reservation = f.reserve(root, f.recipients[0]).await?;
    for op in [
        GiftOperation::Create,
        GiftOperation::Update,
        GiftOperation::SetMeeting,
        GiftOperation::Reserve,
        GiftOperation::Cancel,
        GiftOperation::IssueCode,
        GiftOperation::Claim,
    ] {
        let key = Uuid::new_v4();
        let rid = Uuid::new_v4();
        let request = request(&f, op, root, reservation, key);
        let target = CommandTarget::from_request(&request)
            .map_err(error)?
            .unwrap();
        let mut tx = f.pool.begin().await?;
        target.install_in(&mut tx).await.map_err(error)?;
        let encoded: String =
            sqlx::query_scalar("select current_setting('mcr.gift_live_command_target')")
                .fetch_one(&mut *tx)
                .await?;
        let context: Value = serde_json::from_str(&encoded)?;
        let expected_root = if op == GiftOperation::Create {
            target.target
        } else {
            root
        };
        assert_eq!(
            context,
            json!({"v":1,"actor_id":request.identity().actor,"event_id":f.event,"operation_id":op.id(),"idempotency_key":key,"target_id":target.target,"giveaway_id":expected_root})
        );
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from concert_gift_command_targets where receipt_id=$1)",
        )
        .bind(rid)
        .fetch_one(&mut *tx)
        .await?;
        assert!(!exists);
        sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state) values($1,$2,$3,$4,$5,$6,$7,'PENDING')")
            .bind(rid).bind(request.identity().actor).bind(f.event).bind(op.id()).bind(key)
            .bind(format!("hmac-sha256:v1:sql-fixture:{}","0".repeat(64))).bind(target.target).execute(&mut *tx).await?;
        let actual: Uuid = sqlx::query_scalar(
            "select giveaway_id from concert_gift_command_targets where receipt_id=$1",
        )
        .bind(rid)
        .fetch_one(&mut *tx)
        .await?;
        assert_eq!(actual, expected_root);
        tx.rollback().await?;
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from concert_gift_command_targets where receipt_id=$1)",
        )
        .bind(rid)
        .fetch_one(&f.pool)
        .await?;
        assert!(!exists);
    }
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "private Gift PG fixture; local correlation SQL only"]
async fn missing_or_disabled_migration_fails_before_receipt_admission() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let req = request(
        &f,
        GiftOperation::Create,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let target = CommandTarget::from_request(&req).map_err(error)?.unwrap();
    let mut tx = f.pool.begin().await?;
    sqlx::query(
        "alter table concert_command_receipts disable trigger concert_capture_gift_command_target",
    )
    .execute(&mut *tx)
    .await?;
    assert_eq!(
        target.install_in(&mut tx).await,
        Err(CommandError::Unavailable)
    );
    tx.rollback().await?;
    let mut tx = f.pool.begin().await?;
    sqlx::query("alter table concert_gift_command_targets rename to gift_target_missing_fixture")
        .execute(&mut *tx)
        .await?;
    assert_eq!(
        target.install_in(&mut tx).await,
        Err(CommandError::Unavailable)
    );
    tx.rollback().await?;
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "private Gift PG fixture; local correlation SQL only"]
async fn transaction_context_cannot_be_rebound_and_resets_on_commit() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let root = Uuid::new_v4();
    let reservation = Uuid::new_v4();
    let a = request(&f, GiftOperation::Create, root, reservation, Uuid::new_v4());
    let b = request(&f, GiftOperation::Create, root, reservation, Uuid::new_v4());
    let a = CommandTarget::from_request(&a).map_err(error)?.unwrap();
    let b = CommandTarget::from_request(&b).map_err(error)?.unwrap();
    let mut tx = f.pool.begin().await?;
    a.install_in(&mut tx).await.map_err(error)?;
    a.install_in(&mut tx).await.map_err(error)?;
    assert_eq!(b.install_in(&mut tx).await, Err(CommandError::Denied));
    tx.commit().await?;
    let mut tx = f.pool.begin().await?;
    b.install_in(&mut tx).await.map_err(error)?;
    tx.rollback().await?;
    f.pool.close().await;
    Ok(())
}
