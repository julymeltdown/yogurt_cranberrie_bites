//! Real PostgreSQL producer tests with a deliberately test-only fenced authority.
mod media_support;
use async_trait::async_trait;
use concert_application::{commands::*, media::*};
use concert_service::adapters::out::media::{codec::BoundedImageSanitizer, *};
use image::{DynamicImage, ImageFormat, RgbaImage};
use platform::cpu::WorkerPool;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::{io::Cursor, sync::Arc};
use uuid::Uuid;

struct FixtureAuthority;
#[async_trait]
impl MediaAuthority for FixtureAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        context: Context,
        access: Access,
        owner: Uuid,
        target: Option<Target>,
    ) -> Result<(), CommandError> {
        let row:Option<(i64,bool,bool)>=sqlx::query_as("select revision,allowed,read_target from be24_test_policy where event=$1 and actor=$2 for share").bind(context.event).bind(context.actor).fetch_optional(c).await.map_err(|_|CommandError::Unavailable)?;
        match row {
            Some((revision, true, read_target))
                if revision == context.policy.authority_revision
                    && context.policy.event_revision == 1
                    && context.policy.relationship_revision == 1
                    && (owner == context.actor
                        || (matches!(access, Access::Read(_))
                            && target.is_some()
                            && read_target)) =>
            {
                Ok(())
            }
            _ => Err(CommandError::Denied),
        }
    }
}
fn sanitizer() -> Arc<dyn ImageSanitizer> {
    Arc::new(BoundedImageSanitizer::new(
        WorkerPool::new(1).unwrap().class(0).unwrap(),
    ))
}
fn keys() -> Arc<FingerprintKeys> {
    Arc::new(
        FingerprintKeys::new(SigningKey::new("be24_owned", &[0x56; 32]).unwrap(), vec![]).unwrap(),
    )
}
fn service(p: &PgPool) -> PgMedia {
    PgMedia::new(p.clone(), Arc::new(FixtureAuthority), sanitizer(), keys())
}
async fn fixture() -> anyhow::Result<(PgPool, Context, PgMedia)> {
    let p = media_support::owned_pool().await?;
    sqlx::query("create table if not exists be24_test_policy(event uuid,actor uuid,revision bigint,allowed boolean,read_target boolean,primary key(event,actor))").execute(&p).await?;
    let ctx = Context {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        policy: PolicyFence {
            event_revision: 1,
            relationship_revision: 1,
            authority_revision: 1,
        },
    };
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Owned media fixture','OPEN')").bind(ctx.event).bind(format!("owned-{}",ctx.event)).execute(&p).await?;
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(ctx.event).bind(ctx.actor).execute(&p).await?;
    sqlx::query("insert into be24_test_policy values($1,$2,1,true,false)")
        .bind(ctx.event)
        .bind(ctx.actor)
        .execute(&p)
        .await?;
    let s = service(&p);
    Ok((p, ctx, s))
}
fn encode(format: ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 3, image::Rgba([4, 20, 30, 255])))
        .write_to(&mut out, format)
        .unwrap();
    out.into_inner()
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)))
        }
    }
    !crc
}
fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend(kind);
    out.extend(data);
    let crc = crc32(&out[4..]);
    out.extend(crc.to_be_bytes());
    out
}
fn private_png() -> Vec<u8> {
    let mut v = encode(ImageFormat::Png);
    let end = v.len() - 12;
    v.splice(end..end, chunk(b"tEXt", b"Comment\0BE24_PRIVATE_METADATA"));
    v
}
fn input(bytes: &[u8], mime: Mime, purpose: Purpose) -> InitUpload {
    InitUpload {
        purpose,
        mime,
        byte_length: bytes.len(),
        sha256: Sha256::digest(bytes).into(),
    }
}
async fn upload(
    s: &PgMedia,
    ctx: Context,
    bytes: Vec<u8>,
    mime: Mime,
    purpose: Purpose,
) -> Result<Uuid, CommandError> {
    let r = s
        .initialize(ctx, Uuid::new_v4(), input(&bytes, mime, purpose))
        .await?;
    let id = r.reference.unwrap().resource;
    let r = s.put(ctx, Uuid::new_v4(), id, mime, bytes).await?;
    s.finalize(ctx, Uuid::new_v4(), id, r.reference.unwrap().revision)
        .await?;
    Ok(id)
}
async fn cleanup(p: &PgPool, ctx: Context) {
    sqlx::query("delete from concert_events where event_id=$1")
        .bind(ctx.event)
        .execute(p)
        .await
        .unwrap();
    p.close().await;
}
#[tokio::test]
async fn durable_lifecycle_metadata_removal_receipt_replay_and_headers() -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let bytes = private_png();
    let key = Uuid::new_v4();
    let first = s
        .initialize(c, key, input(&bytes, Mime::Png, Purpose::Post))
        .await?;
    let id = first.reference.unwrap().resource;
    assert_eq!(
        s.initialize(c, key, input(&bytes, Mime::Png, Purpose::Post))
            .await?,
        first
    );
    assert!(s.read(c, id).await.is_err());
    assert_eq!(s.intent(c, id).await?.0, 1);
    assert_eq!(first.result_status, Some(200));
    let put_key = Uuid::new_v4();
    let put = s.put(c, put_key, id, Mime::Png, bytes.clone()).await?;
    assert_eq!(put.reference.unwrap().revision, 2);
    assert!(s.read(c, id).await.is_err());
    assert_eq!(s.put(c, put_key, id, Mime::Png, bytes).await?, put);
    assert!(s.put(c, put_key, id, Mime::Jpeg, vec![1]).await.is_err());
    let key = Uuid::new_v4();
    let done = s.finalize(c, key, id, 2).await?;
    // Simulated lost response: reconstruct service, retry the SAME key, observe same receipt.
    let restarted = service(&p);
    assert_eq!(restarted.finalize(c, key, id, 2).await?, done);
    assert_eq!(done.reference.unwrap().revision, 3);
    assert_eq!(
        s.recover(c, key, id, MediaOperation::Finalize).await?,
        Some(done.clone())
    );
    assert!(s
        .recover(
            Context {
                actor: Uuid::new_v4(),
                ..c
            },
            key,
            id,
            MediaOperation::Finalize
        )
        .await
        .is_err());
    let read = s.read(c, id).await?;
    assert_eq!((read.metadata.width, read.metadata.height), (4, 3));
    assert!(!read
        .bytes
        .windows(21)
        .any(|v| v == b"BE24_PRIVATE_METADATA"));
    assert_eq!(read.headers()[1], ("cache-control", "private, no-store"));
    assert_eq!(read.headers()[2], ("x-content-type-options", "nosniff"));
    let raw: bool = sqlx::query_scalar(
        "select staged is null and sanitized is not null from concert_media where media_id=$1",
    )
    .bind(id)
    .fetch_one(&p)
    .await?;
    assert!(raw);
    assert!(s.finalize(c, Uuid::new_v4(), id, 2).await.is_err());
    let mut changed = input(b"different", Mime::Png, Purpose::Post);
    changed.byte_length = 5;
    assert_eq!(
        s.initialize(c, first.scope.key, changed).await,
        Err(CommandError::Conflict)
    );
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn rejects_forged_mime_bad_digest_and_foreign_ids() -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let bytes = encode(ImageFormat::Png);
    let r = s
        .initialize(c, Uuid::new_v4(), input(&bytes, Mime::Jpeg, Purpose::Post))
        .await?;
    let id = r.reference.unwrap().resource;
    assert!(s
        .put(c, Uuid::new_v4(), id, Mime::Png, bytes.clone())
        .await
        .is_err());
    assert!(s
        .put(c, Uuid::new_v4(), id, Mime::Jpeg, b"bad-digest".to_vec())
        .await
        .is_err());
    s.put(c, Uuid::new_v4(), id, Mime::Jpeg, bytes).await?;
    assert_eq!(
        s.finalize(c, Uuid::new_v4(), id, 2).await,
        Err(CommandError::InvalidInput)
    );
    let foreign = Context {
        actor: Uuid::new_v4(),
        ..c
    };
    assert!(s.finalize(foreign, Uuid::new_v4(), id, 2).await.is_err());
    assert!(s.read(foreign, id).await.is_err());
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_media where media_id=$1 and state='READY'",
    )
    .bind(id)
    .fetch_one(&p)
    .await?;
    assert_eq!(count, 0);
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn oversized_and_expanding_dimensions_never_publish() -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let invalid = InitUpload {
        byte_length: MAX_BYTES + 1,
        ..input(b"x", Mime::Png, Purpose::Post)
    };
    assert_eq!(
        s.initialize(c, Uuid::new_v4(), invalid).await,
        Err(CommandError::InvalidInput)
    );
    let mut bomb = encode(ImageFormat::Png);
    bomb[16..20].copy_from_slice(&50000u32.to_be_bytes());
    let crc = crc32(&bomb[12..29]);
    bomb[29..33].copy_from_slice(&crc.to_be_bytes());
    let r = s
        .initialize(c, Uuid::new_v4(), input(&bomb, Mime::Png, Purpose::Post))
        .await?;
    let id = r.reference.unwrap().resource;
    assert!(s
        .put(c, Uuid::new_v4(), id, Mime::Png, vec![0; MAX_BYTES + 1])
        .await
        .is_err());
    s.put(c, Uuid::new_v4(), id, Mime::Png, bomb).await?;
    assert_eq!(
        s.finalize(c, Uuid::new_v4(), id, 2).await,
        Err(CommandError::InvalidInput)
    );
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn stale_revoked_and_unavailable_authority_deny_replay_and_reads() -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let id = upload(&s, c, encode(ImageFormat::Png), Mime::Png, Purpose::Report).await?;
    sqlx::query("update be24_test_policy set revision=2 where event=$1")
        .bind(c.event)
        .execute(&p)
        .await?;
    assert!(s.read(c, id).await.is_err());
    // A fresh current policy can read an existing READY image; stored upload fence is for writes.
    assert!(s
        .read(
            Context {
                policy: PolicyFence {
                    authority_revision: 2,
                    ..c.policy
                },
                ..c
            },
            id
        )
        .await
        .is_ok());
    assert!(s.finalize(c, Uuid::new_v4(), id, 2).await.is_err());
    sqlx::query("update be24_test_policy set allowed=false where event=$1")
        .bind(c.event)
        .execute(&p)
        .await?;
    assert!(s
        .read(
            Context {
                policy: PolicyFence {
                    authority_revision: 2,
                    ..c.policy
                },
                ..c
            },
            id
        )
        .await
        .is_err());
    let unavailable = PgMedia::new(
        p.clone(),
        Arc::new(UnavailableMediaAuthority),
        sanitizer(),
        keys(),
    );
    assert_eq!(
        unavailable
            .initialize(c, Uuid::new_v4(), input(b"x", Mime::Png, Purpose::Report))
            .await,
        Err(CommandError::Unavailable)
    );
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn target_linkage_is_atomic_purpose_bound_and_private_reads_recheck_policy(
) -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let id = upload(
        &s,
        c,
        encode(ImageFormat::Png),
        Mime::Png,
        Purpose::Giveaway,
    )
    .await?;
    let target = Target {
        resource: Uuid::new_v4(),
        purpose: Purpose::Giveaway,
    };
    let mut tx = p.begin().await?;
    assert!(s
        .link_owned_in(
            &mut tx,
            c,
            id,
            Target {
                purpose: Purpose::Post,
                ..target
            }
        )
        .await
        .is_err());
    tx.rollback().await?;
    let mut tx = p.begin().await?;
    s.link_owned_in(&mut tx, c, id, target).await?;
    tx.rollback().await?;
    let no_target: bool =
        sqlx::query_scalar("select target_id is null from concert_media where media_id=$1")
            .bind(id)
            .fetch_one(&p)
            .await?;
    assert!(no_target);
    let mut tx = p.begin().await?;
    let a = s.link_owned_in(&mut tx, c, id, target).await?;
    let b = s.link_owned_in(&mut tx, c, id, target).await?;
    assert_eq!(a, b);
    tx.commit().await?;
    let viewer = Context {
        actor: Uuid::new_v4(),
        ..c
    };
    sqlx::query("insert into be24_test_policy values($1,$2,1,true,false)")
        .bind(c.event)
        .bind(viewer.actor)
        .execute(&p)
        .await?;
    assert!(s.read(viewer, id).await.is_err());
    sqlx::query("update be24_test_policy set read_target=true where event=$1 and actor=$2")
        .bind(c.event)
        .bind(viewer.actor)
        .execute(&p)
        .await?;
    assert!(s.read(viewer, id).await.is_ok());
    sqlx::query("update be24_test_policy set allowed=false where event=$1 and actor=$2")
        .bind(c.event)
        .bind(viewer.actor)
        .execute(&p)
        .await?;
    assert!(s.read(viewer, id).await.is_err());
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn privacy_erasure_clears_both_staged_and_ready_and_prevents_resurrection(
) -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let ready = upload(&s, c, encode(ImageFormat::Png), Mime::Png, Purpose::Report).await?;
    let bytes = encode(ImageFormat::Png);
    let r = s
        .initialize(c, Uuid::new_v4(), input(&bytes, Mime::Png, Purpose::Post))
        .await?;
    let pending = r.reference.unwrap().resource;
    s.put(c, Uuid::new_v4(), pending, Mime::Png, bytes).await?;
    let mut tx = p.begin().await?;
    assert_eq!(PgMedia::erase_owner_in(&mut tx, c.event, c.actor).await?, 2);
    tx.commit().await?;
    assert!(s.read(c, ready).await.is_err());
    assert!(s.finalize(c, Uuid::new_v4(), pending, 2).await.is_err());
    let remaining:i64=sqlx::query_scalar("select count(*) from concert_media where event_id=$1 and (staged is not null or sanitized is not null or state<>'DELETED')").bind(c.event).fetch_one(&p).await?;
    assert_eq!(remaining, 0);
    assert!(s
        .initialize(c, r.scope.key, input(b"other", Mime::Png, Purpose::Post))
        .await
        .is_err());
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn all_formats_decode_reencode_and_animation_is_rejected() -> anyhow::Result<()> {
    let codec = sanitizer();
    for (mime, format) in [
        (Mime::Png, ImageFormat::Png),
        (Mime::Webp, ImageFormat::WebP),
    ] {
        let clean = codec.sanitize(encode(format), mime).await?;
        assert_eq!((clean.width, clean.height), (4, 3));
        assert!(!clean.bytes.is_empty());
    }
    let mut jpeg = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image::RgbImage::from_pixel(4, 3, image::Rgb([4, 5, 6])))
        .write_to(&mut jpeg, ImageFormat::Jpeg)?;
    let mut jpeg = jpeg.into_inner();
    let private = b"BE24_PRIVATE_JPEG_COMMENT";
    let mut comment = vec![255, 254];
    comment.extend(((private.len() + 2) as u16).to_be_bytes());
    comment.extend(private);
    jpeg.splice(2..2, comment);
    let clean = codec.sanitize(jpeg, Mime::Jpeg).await?;
    assert!(!clean.bytes.windows(private.len()).any(|v| v == private));
    let mut png = encode(ImageFormat::Png);
    let mut actl = 1u32.to_be_bytes().to_vec();
    actl.extend(0u32.to_be_bytes());
    png.splice(33..33, chunk(b"acTL", &actl));
    assert!(codec.sanitize(png, Mime::Png).await.is_err());
    // Both axes fit 4096 but 4001*4001 exceeds the separate 16,000,000 pixel cap.
    let mut pixels = encode(ImageFormat::Png);
    pixels[16..20].copy_from_slice(&4001u32.to_be_bytes());
    pixels[20..24].copy_from_slice(&4001u32.to_be_bytes());
    let crc = crc32(&pixels[12..29]);
    pixels[29..33].copy_from_slice(&crc.to_be_bytes());
    assert!(codec.sanitize(pixels, Mime::Png).await.is_err());
    assert!(codec.sanitize(b"GIF89a".to_vec(), Mime::Png).await.is_err());
    Ok(())
}
#[tokio::test]
async fn policy_revocation_waits_for_protected_read_transaction() -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let id = upload(&s, c, encode(ImageFormat::Png), Mime::Png, Purpose::Post).await?;
    let mut policy = p.begin().await?;
    sqlx::query("update be24_test_policy set allowed=false where event=$1")
        .bind(c.event)
        .execute(&mut *policy)
        .await?;
    let read = tokio::spawn(async move { s.read(c, id).await.is_err() });
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(!read.is_finished());
    policy.commit().await?;
    assert!(read.await?);
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn upload_expiry_and_orphan_cleanup_delete_bytes_in_bounded_batches() -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    // Expiry timestamps are immutable in product code: fixture inserts already expired rows.
    let id = Uuid::new_v4();
    let bytes = encode(ImageFormat::Png);
    sqlx::query("insert into concert_media(media_id,event_id,owner_id,purpose,content_type,declared_bytes,input_sha256,policy_event_revision,policy_relationship_revision,policy_authority_revision,state,staged,created_at,expires_at,orphan_at) values($1,$2,$3,'POST','image/png',$4,$5,1,1,1,'UPLOADED',$6,clock_timestamp()-interval '2 days',clock_timestamp()-interval '1 day',clock_timestamp()-interval '1 hour')")
 .bind(id).bind(c.event).bind(c.actor).bind(bytes.len() as i32).bind(Sha256::digest(&bytes).as_slice()).bind(bytes).execute(&p).await?;
    assert_eq!(
        s.finalize(c, Uuid::new_v4(), id, 1).await,
        Err(CommandError::Expired)
    );
    assert!(s.cleanup(0).await.is_err());
    assert!(s.cleanup(101).await.is_err());
    assert_eq!(s.cleanup(1).await?, 1);
    let cleared:bool=sqlx::query_scalar("select state='DELETED' and staged is null and sanitized is null from concert_media where media_id=$1").bind(id).fetch_one(&p).await?;
    assert!(cleared);
    cleanup(&p, c).await;
    Ok(())
}
const RESTART_EVENT: Uuid = Uuid::from_u128(0xa0240000000000000000000000000001);
const RESTART_ACTOR: Uuid = Uuid::from_u128(0xa0240000000000000000000000000002);
fn restart_context() -> Context {
    Context {
        actor: RESTART_ACTOR,
        event: RESTART_EVENT,
        policy: PolicyFence {
            event_revision: 1,
            relationship_revision: 1,
            authority_revision: 1,
        },
    }
}
#[tokio::test]
async fn durable_restart_seed() -> anyhow::Result<()> {
    let p = media_support::owned_pool().await?;
    sqlx::query("create table if not exists be24_test_policy(event uuid,actor uuid,revision bigint,allowed boolean,read_target boolean,primary key(event,actor))").execute(&p).await?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,'be24-restart','Owned restart','OPEN')").bind(RESTART_EVENT).execute(&p).await?;
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(RESTART_EVENT).bind(RESTART_ACTOR).execute(&p).await?;
    sqlx::query("insert into be24_test_policy values($1,$2,1,true,false)")
        .bind(RESTART_EVENT)
        .bind(RESTART_ACTOR)
        .execute(&p)
        .await?;
    let s = service(&p);
    upload(
        &s,
        restart_context(),
        private_png(),
        Mime::Png,
        Purpose::Post,
    )
    .await?;
    p.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "run by owned smoke harness after an actual PostgreSQL restart"]
async fn durable_restart_verify() -> anyhow::Result<()> {
    let p = media_support::owned_pool().await?;
    let s = service(&p);
    let id: Uuid = sqlx::query_scalar(
        "select media_id from concert_media where event_id=$1 and state='READY'",
    )
    .bind(RESTART_EVENT)
    .fetch_one(&p)
    .await?;
    let read = s.read(restart_context(), id).await?;
    assert_eq!((read.metadata.width, read.metadata.height), (4, 3));
    assert!(!read
        .bytes
        .windows(21)
        .any(|v| v == b"BE24_PRIVATE_METADATA"));
    let receipts: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where event_id=$1 and state='SUCCEEDED'",
    )
    .bind(RESTART_EVENT)
    .fetch_one(&p)
    .await?;
    assert_eq!(receipts, 3);
    cleanup(&p, restart_context()).await;
    Ok(())
}

#[test]
fn streaming_bound_and_digest_reject_overrun_truncation_and_poisoned_reuse() {
    use concert_service::adapters::out::media::body::UploadBody;
    let intent = input(b"hello", Mime::Png, Purpose::Post);
    let mut body = UploadBody::new(&intent).unwrap();
    body.push(b"he").unwrap();
    body.push(b"llo").unwrap();
    assert_eq!(body.finish().unwrap(), b"hello");
    let mut body = UploadBody::new(&intent).unwrap();
    assert!(body.push(b"hello!").is_err());
    assert!(body.push(b"hello").is_err());
    assert!(body.finish().is_err());
    let mut body = UploadBody::new(&intent).unwrap();
    body.push(b"hell").unwrap();
    assert!(body.finish().is_err());
    let mut body = UploadBody::new(&intent).unwrap();
    body.push(b"abcde").unwrap();
    assert!(body.finish().is_err());
}
struct SlowSanitizer;
#[async_trait]
impl ImageSanitizer for SlowSanitizer {
    async fn sanitize(&self, bytes: Vec<u8>, mime: Mime) -> Result<Sanitized, CommandError> {
        tokio::time::sleep(std::time::Duration::from_millis(180)).await;
        sanitizer().sanitize(bytes, mime).await
    }
}
#[tokio::test]
async fn expiry_during_sanitize_cannot_publish_ready() -> anyhow::Result<()> {
    let (p, c, _) = fixture().await?;
    let s = PgMedia::new(
        p.clone(),
        Arc::new(FixtureAuthority),
        Arc::new(SlowSanitizer),
        keys(),
    );
    let id = Uuid::new_v4();
    let bytes = encode(ImageFormat::Png);
    sqlx::query("insert into concert_media(media_id,event_id,owner_id,purpose,content_type,declared_bytes,input_sha256,policy_event_revision,policy_relationship_revision,policy_authority_revision,state,staged,created_at,expires_at) values($1,$2,$3,'POST','image/png',$4,$5,1,1,1,'UPLOADED',$6,clock_timestamp()-interval '1 second',clock_timestamp()+interval '120 milliseconds')")
 .bind(id).bind(c.event).bind(c.actor).bind(bytes.len() as i32).bind(Sha256::digest(&bytes).as_slice()).bind(bytes).execute(&p).await?;
    assert_eq!(
        s.finalize(c, Uuid::new_v4(), id, 1).await,
        Err(CommandError::Expired)
    );
    let state: String = sqlx::query_scalar("select state from concert_media where media_id=$1")
        .bind(id)
        .fetch_one(&p)
        .await?;
    assert_eq!(state, "UPLOADED");
    let receipt: i64 =
        sqlx::query_scalar("select count(*) from concert_command_receipts where target_id=$1")
            .bind(id)
            .fetch_one(&p)
            .await?;
    assert_eq!(receipt, 0);
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn owner_quota_and_database_guards_preserve_private_publication() -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let id = upload(&s, c, encode(ImageFormat::Png), Mime::Png, Purpose::Post).await?;
    // Publishing a new copy over READY, retargeting ownership, or resurrecting raw content
    // is rejected even if a future producer accidentally bypasses application checks.
    assert!(sqlx::query("update concert_media set sanitized='untrusted'::bytea,revision=revision+1 where media_id=$1").bind(id).execute(&p).await.is_err());
    assert!(sqlx::query(
        "update concert_media set owner_id=$2,revision=revision+1 where media_id=$1"
    )
    .bind(id)
    .bind(Uuid::new_v4())
    .execute(&p)
    .await
    .is_err());
    for _ in 1..16 {
        s.initialize(c, Uuid::new_v4(), input(b"x", Mime::Png, Purpose::Post))
            .await?;
    }
    assert_eq!(
        s.initialize(c, Uuid::new_v4(), input(b"x", Mime::Png, Purpose::Post))
            .await,
        Err(CommandError::Capacity)
    );
    let count: i64 = sqlx::query_scalar("select count(*) from concert_media where event_id=$1")
        .bind(c.event)
        .fetch_one(&p)
        .await?;
    assert_eq!(count, 16);
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn competing_finalize_same_key_converges_on_one_durable_receipt() -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let bytes = encode(ImageFormat::Png);
    let id = s
        .initialize(c, Uuid::new_v4(), input(&bytes, Mime::Png, Purpose::Post))
        .await?
        .reference
        .unwrap()
        .resource;
    s.put(c, Uuid::new_v4(), id, Mime::Png, bytes).await?;
    let key = Uuid::new_v4();
    let (a, b) = tokio::join!(s.finalize(c, key, id, 2), s.finalize(c, key, id, 2));
    assert_eq!(a?, b?);
    let revision: i64 = sqlx::query_scalar("select revision from concert_media where media_id=$1")
        .bind(id)
        .fetch_one(&p)
        .await?;
    assert_eq!(revision, 3);
    let receipts:i64=sqlx::query_scalar("select count(*) from concert_command_receipts where target_id=$1 and operation_id='finalizeConcertUpload'").bind(id).fetch_one(&p).await?;
    assert_eq!(receipts, 1);
    cleanup(&p, c).await;
    Ok(())
}
#[tokio::test]
async fn shorter_caller_sql_deadline_is_preserved() -> anyhow::Result<()> {
    let (p, c, s) = fixture().await?;
    let id = upload(
        &s,
        c,
        encode(ImageFormat::Png),
        Mime::Png,
        Purpose::Giveaway,
    )
    .await?;
    let mut tx = p.begin().await?;
    sqlx::query("set local lock_timeout='100ms'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("set local statement_timeout='200ms'")
        .execute(&mut *tx)
        .await?;
    s.link_owned_in(
        &mut tx,
        c,
        id,
        Target {
            resource: Uuid::new_v4(),
            purpose: Purpose::Giveaway,
        },
    )
    .await?;
    let lock: String = sqlx::query_scalar("show lock_timeout")
        .fetch_one(&mut *tx)
        .await?;
    let statement: String = sqlx::query_scalar("show statement_timeout")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(lock, "100ms");
    assert_eq!(statement, "200ms");
    tx.rollback().await?;
    cleanup(&p, c).await;
    Ok(())
}
