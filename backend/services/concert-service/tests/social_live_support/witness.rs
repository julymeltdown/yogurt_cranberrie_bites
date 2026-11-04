//! Local SQL ledger fixtures only; never an authenticated Root/owner grant.
use crate::{
    adapters::out::relationship_authority::guarded::{
        proto::TransactionStatus, ProducerTransaction as NativeIdentity,
    },
    product_rpc::{scope_ledger::ProducerTransaction, settlement::NativeTransactionResolver},
    social_live::witness::SocialNativeResolver,
};
use chrono::Utc;
use platform::actor_delegation::owner_binding::OwnerBinding;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;
struct Fixture {
    p: PgPool,
    b: OwnerBinding,
    instance: Uuid,
    scope: Uuid,
    key: Option<Uuid>,
}
impl Fixture {
    async fn new(mutation: bool) -> Self {
        let p = crate::owned::pool().await;
        let actor = Uuid::new_v4();
        let event = Uuid::new_v4();
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Witness fixture','LIVE')").bind(event).bind(event.to_string()).execute(&p).await.unwrap();
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(event).bind(actor).execute(&p).await.unwrap();
        let operation = if mutation {
            "setConcertHandle"
        } else {
            "listOwnConcertHandles"
        };
        let b = OwnerBinding {
            actor_id: actor,
            event_id: event,
            session_family_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            operation: operation.into(),
            expires_unix_micros: (Utc::now() + chrono::Duration::seconds(60)).timestamp_micros(),
            parent_digest: [11; 32],
            parent_body_digest: [12; 32],
            parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
            http_method: if mutation { "PUT" } else { "GET" }.into(),
            http_target: format!(
                "/api/concert/events/{event}/me/social-handles{}",
                if mutation { "/INSTAGRAM" } else { "" }
            ),
            body_digest: [13; 32],
        };
        let f = Self {
            p,
            b,
            instance: Uuid::new_v4(),
            scope: Uuid::new_v4(),
            key: mutation.then(Uuid::new_v4),
        };
        sqlx::query("insert into concert_product_owner_scopes(instance_id,scope_id,binding_digest,binding,delivery_secret_hash,serialized_at) values($1,$2,$3,$4,$5,clock_timestamp())")
            .bind(f.instance).bind(f.scope).bind(f.b.digest().unwrap().as_slice()).bind(sqlx::types::Json(&f.b)).bind([15u8;32].as_slice()).execute(&f.p).await.unwrap();
        f
    }
    async fn register(&self, c: &mut PgConnection) -> ProducerTransaction {
        let mut h = Sha256::new();
        h.update(b"MCR social transaction v1\0");
        h.update(self.b.digest().unwrap());
        let effect = h.finalize().into();
        let n = NativeIdentity::capture_concert_in(
            c,
            effect,
            self.instance,
            self.b.request_id.as_bytes().to_vec(),
        )
        .await
        .unwrap();
        let i = ProducerTransaction {
            producer: "CONCERT".into(),
            system_identifier: n.system_identifier,
            database_oid: n.database_oid,
            full_xid: n.full_xid,
            backend_pid: n.backend_pid,
            backend_started_unix_micros: n.backend_started.timestamp_micros(),
            effect_binding_digest: effect,
            producer_instance: self.instance,
            settlement_reference: n.settlement_reference,
        };
        sqlx::query("insert into concert_product_owner_transactions(instance_id,scope_id,binding_digest,identity_digest,identity,auth_registration_id,friend_registration_id,confirmed_at) values($1,$2,$3,$4,$5,$6,$7,clock_timestamp())")
            .bind(self.instance).bind(self.scope).bind(self.b.digest().unwrap().as_slice()).bind(i.digest().unwrap().as_slice()).bind(sqlx::types::Json(&i)).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&self.p).await.unwrap();
        i
    }
    async fn insert(
        &self,
        c: &mut PgConnection,
        i: &ProducerTransaction,
        mutation: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("insert into concert_social_effect_witnesses(producer_instance,scope_id,identity_digest,guard_binding_digest,effect_binding_digest,request_id,actor_id,event_id,session_family_id,operation,parent_digest,body_digest,native_identity,resource_id,resource_revision,effect_kind,idempotency_key) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$7,$14,$15,$16)")
            .bind(self.instance).bind(self.scope).bind(i.digest().unwrap().as_slice()).bind(self.b.digest().unwrap().as_slice()).bind(i.effect_binding_digest.as_slice()).bind(self.b.request_id).bind(self.b.actor_id).bind(self.b.event_id).bind(self.b.session_family_id).bind(&self.b.operation).bind(self.b.parent_digest.as_slice()).bind(self.b.body_digest.as_slice()).bind(sqlx::types::Json(i)).bind(if mutation{1i64}else{0}).bind(if mutation{"MUTATION"}else{"PROJECTION"}).bind(self.key).execute(c).await?;
        Ok(())
    }
    async fn finish(&self) {
        sqlx::query("update concert_product_owner_scopes set phase='SEALED',sealed_at=clock_timestamp() where instance_id=$1 and scope_id=$2").bind(self.instance).bind(self.scope).execute(&self.p).await.unwrap();
        sqlx::query("update concert_product_owner_scopes set phase='ROOT_FINISHED',root_finished_at=clock_timestamp() where instance_id=$1 and scope_id=$2").bind(self.instance).bind(self.scope).execute(&self.p).await.unwrap();
    }
    async fn resolve(
        &self,
        i: &ProducerTransaction,
    ) -> crate::product_rpc::settlement::NativeTransactionResolution {
        SocialNativeResolver::new(self.p.clone(), self.instance)
            .unwrap()
            .resolve(&self.b, i)
            .await
            .unwrap()
    }
    async fn pending_receipt(&self, c: &mut PgConnection) {
        sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state) values($1,$2,$3,$4,$5,$6,$2,'PENDING')")
            .bind(Uuid::new_v4()).bind(self.b.actor_id).bind(self.b.event_id).bind(&self.b.operation).bind(self.key).bind(format!("hmac-sha256:v1:fixture:{}","a".repeat(64))).execute(c).await.unwrap();
    }
    async fn effect(&self, c: &mut PgConnection) {
        // Synthetic source fixture only. Actual production authority comes from
        // its registered owner scope; the retained deferred SQL guard still runs.
        sqlx::query("select set_config('mcr.relationship_authority_until',$1,true)")
            .bind(
                chrono::DateTime::<Utc>::from_timestamp_micros(self.b.expires_unix_micros)
                    .unwrap()
                    .to_rfc3339(),
            )
            .execute(&mut *c)
            .await
            .unwrap();
        sqlx::query("insert into concert_social_handles(event_id,user_id,platform,key_id,nonce,ciphertext) values($1,$2,'INSTAGRAM','fixture',decode(repeat('01',12),'hex'),decode(repeat('02',17),'hex'))")
            .bind(self.b.event_id).bind(self.b.actor_id).execute(c).await.unwrap();
    }
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn actual_native_outcomes_never_promote_unwitnessed_commit() {
    let f = Fixture::new(false).await;
    let mut tx = f.p.begin().await.unwrap();
    let identity = f.register(&mut tx).await;
    assert_eq!(
        f.resolve(&identity).await.status,
        TransactionStatus::InProgress
    );
    tx.commit().await.unwrap();
    f.finish().await;
    let result = f.resolve(&identity).await;
    assert_eq!(result.status, TransactionStatus::Unknown);
    assert!(result.receipt_evidence.is_empty());
    let mut b = f.b.clone();
    b.body_digest = [19; 32];
    let resolver = SocialNativeResolver::new(f.p.clone(), f.instance).unwrap();
    assert_eq!(
        resolver.resolve(&b, &identity).await.unwrap().status,
        TransactionStatus::Unknown
    );
    f.p.close().await;
    let f = Fixture::new(false).await;
    let mut tx = f.p.begin().await.unwrap();
    let identity = f.register(&mut tx).await;
    tx.rollback().await.unwrap();
    f.finish().await;
    assert_eq!(
        f.resolve(&identity).await.status,
        TransactionStatus::Aborted
    );
    f.p.close().await;
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn completed_projection_is_exact_immutable_native_evidence() {
    let f = Fixture::new(false).await;
    let mut tx = f.p.begin().await.unwrap();
    let identity = f.register(&mut tx).await;
    let _: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_handles where event_id=$1 and user_id=$2",
    )
    .bind(f.b.event_id)
    .bind(f.b.actor_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    f.insert(&mut tx, &identity, false).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        f.resolve(&identity).await.status,
        TransactionStatus::Unknown,
        "no unfinished root commit proof"
    );
    f.finish().await;
    let result = f.resolve(&identity).await;
    assert_eq!(result.status, TransactionStatus::Committed);
    assert_eq!(result.receipt_evidence.len(), 32);
    for sql in [
        "update concert_social_effect_witnesses set resource_revision=1 where producer_instance=$1",
        "delete from concert_social_effect_witnesses where producer_instance=$1",
    ] {
        assert!(sqlx::query(sql)
            .bind(f.instance)
            .execute(&f.p)
            .await
            .is_err());
    }
    let mut swapped = identity.clone();
    swapped.backend_pid += 1;
    assert_eq!(f.resolve(&swapped).await.status, TransactionStatus::Unknown);
    let mut swapped = identity.clone();
    swapped.database_oid += 1;
    assert_eq!(f.resolve(&swapped).await.status, TransactionStatus::Unknown);
    f.p.close().await;
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn mutation_witness_requires_actual_effect_and_successful_receipt_at_commit() {
    let f = Fixture::new(true).await;
    let mut tx = f.p.begin().await.unwrap();
    let identity = f.register(&mut tx).await;
    f.pending_receipt(&mut tx).await;
    sqlx::query("savepoint before_early_witness")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(f.insert(&mut tx, &identity, true).await.is_err());
    sqlx::query("rollback to savepoint before_early_witness")
        .execute(&mut *tx)
        .await
        .unwrap();
    f.effect(&mut tx).await;
    f.insert(&mut tx, &identity, true).await.unwrap();
    assert!(
        tx.commit().await.is_err(),
        "pending receipt cannot certify a commit"
    );
    f.finish().await;
    assert_eq!(
        f.resolve(&identity).await.status,
        TransactionStatus::Aborted
    );
    f.p.close().await;
    let f = Fixture::new(true).await;
    let mut tx = f.p.begin().await.unwrap();
    let identity = f.register(&mut tx).await;
    f.pending_receipt(&mut tx).await;
    f.effect(&mut tx).await;
    f.insert(&mut tx, &identity, true).await.unwrap();
    sqlx::query("update concert_command_receipts set state='SUCCEEDED',resource_id=actor_id,resource_revision=1,result_status=200 where actor_id=$1 and event_id=$2 and idempotency_key=$3").bind(f.b.actor_id).bind(f.b.event_id).bind(f.key).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let own: bool = sqlx::query_scalar("select public.concert_social_own_visible_xmin(xmin) from concert_social_handles where event_id=$1 and user_id=$2")
        .bind(f.b.event_id).bind(f.b.actor_id).fetch_one(&f.p).await.unwrap();
    assert!(
        !own,
        "a previously committed visible row is not a current effect"
    );
    for special in ["0", "1", "2"] {
        let own: bool =
            sqlx::query_scalar("select public.concert_social_own_visible_xmin($1::text::xid)")
                .bind(special)
                .fetch_one(&f.p)
                .await
                .unwrap();
        assert!(
            !own,
            "invalid/bootstrap/frozen xmin is never current mutation evidence"
        );
    }
    f.finish().await;
    assert_eq!(
        f.resolve(&identity).await.status,
        TransactionStatus::Committed
    );
    f.p.close().await;
}
#[tokio::test]
#[ignore = "owned social_live native runner required"]
async fn actual_subtransaction_effect_and_witness_disappear_together_on_savepoint_rollback() {
    let f = Fixture::new(true).await;
    let mut tx = f.p.begin().await.unwrap();
    let identity = f.register(&mut tx).await;
    f.pending_receipt(&mut tx).await;
    sqlx::query("savepoint command_effects")
        .execute(&mut *tx)
        .await
        .unwrap();
    f.effect(&mut tx).await;
    let own:bool=sqlx::query_scalar("select xmin::text<>mod(pg_current_xact_id()::text::numeric,4294967296)::text and public.concert_social_own_visible_xmin(xmin) from concert_social_handles where event_id=$1 and user_id=$2")
        .bind(f.b.event_id).bind(f.b.actor_id).fetch_one(&mut *tx).await.unwrap();
    assert!(
        own,
        "actual visible savepoint write has a different xmin from the top xid"
    );
    f.insert(&mut tx, &identity, true).await.unwrap();
    sqlx::query("update concert_command_receipts set state='SUCCEEDED',resource_id=actor_id,resource_revision=1,result_status=200 where actor_id=$1 and event_id=$2 and idempotency_key=$3").bind(f.b.actor_id).bind(f.b.event_id).bind(f.key).execute(&mut *tx).await.unwrap();
    sqlx::query("rollback to savepoint command_effects")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    f.finish().await;
    assert_eq!(
        f.resolve(&identity).await.status,
        TransactionStatus::Unknown,
        "raw committed parent cannot certify a rolled back effect"
    );
    let n: i64 = sqlx::query_scalar(
        "select count(*) from concert_social_effect_witnesses where producer_instance=$1",
    )
    .bind(f.instance)
    .fetch_one(&f.p)
    .await
    .unwrap();
    assert_eq!(n, 0);
    f.p.close().await;
}
