//! Synthetic owner proof + owned real PostgreSQL/codec, never production authority.
#[path = "../support/meeting/mod.rs"]
mod meeting;
use async_trait::async_trait;
use concert_application::{beacon::*, commands::*, media::*};
use concert_service::{
    adapters::out::media::{codec::BoundedImageSanitizer, MediaAuthority, PgMedia},
    meeting::MeetingAuthority,
    meeting_photo::{wire::*, PhotoBeacons, PhotoFrame, PhotoMutation, PhotoTransactionGuard},
};
use image::{DynamicImage, ImageFormat, RgbaImage};
pub use meeting::{owned_pool, setup, Fixture, SyntheticAuthority};
use platform::cpu::WorkerPool;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::{io::Cursor, sync::Arc};
use uuid::Uuid;
#[derive(Clone, Copy)]
pub struct FixtureGuard;
#[async_trait]
impl PhotoTransactionGuard for FixtureGuard {
    async fn register_in(
        &self,
        c: &mut PgConnection,
        f: &PhotoFrame,
    ) -> Result<Context, CommandError> {
        SyntheticAuthority
            .actor_in(c, f.actor)
            .await
            .map_err(|_| CommandError::Denied)?;
        // Test-only fixture guard retains the synthetic authority row through commit.
        Ok(context(f.actor))
    }
}
pub fn context(actor: MeetingActor) -> Context {
    Context {
        actor: actor.actor,
        event: actor.event,
        policy: PolicyFence {
            event_revision: 1,
            relationship_revision: 1,
            authority_revision: 1,
        },
    }
}
struct UploadAuthority;
#[async_trait]
impl MediaAuthority for UploadAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        ctx: Context,
        access: Access,
        owner: Uuid,
        target: Option<Target>,
    ) -> Result<(), CommandError> {
        SyntheticAuthority
            .actor_in(
                c,
                MeetingActor {
                    actor: ctx.actor,
                    event: ctx.event,
                },
            )
            .await
            .map_err(|_| CommandError::Denied)?;
        if owner != ctx.actor
            || target.is_some()
            || matches!(access, Access::Link(_) | Access::Delete(_))
        {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
}
pub fn keys() -> Arc<FingerprintKeys> {
    Arc::new(
        FingerprintKeys::new(
            SigningKey::new("meeting-photo-test", &[89; 32]).unwrap(),
            vec![],
        )
        .unwrap(),
    )
}
pub fn codec() -> Arc<dyn ImageSanitizer> {
    Arc::new(BoundedImageSanitizer::new(
        WorkerPool::new(1).unwrap().class(0).unwrap(),
    ))
}
pub type Service = PhotoBeacons<SyntheticAuthority, FixtureGuard>;
pub fn service(pool: &PgPool) -> Arc<Service> {
    Arc::new(PhotoBeacons::new(
        pool.clone(),
        Arc::new(SyntheticAuthority),
        Arc::new(FixtureGuard),
        codec(),
        keys(),
    ))
}
pub async fn fixture() -> anyhow::Result<(PgPool, Fixture, Arc<Service>)> {
    let pool = owned_pool().await?;
    let f = setup(&pool).await?;
    for user in [f.owner, f.peer] {
        sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(f.event).bind(user).execute(&pool).await?;
    }
    let s = service(&pool);
    Ok((pool, f, s))
}
pub async fn upload(
    pool: &PgPool,
    f: &Fixture,
    actor: Uuid,
    purpose: Purpose,
) -> anyhow::Result<Uuid> {
    let media = PgMedia::new(pool.clone(), Arc::new(UploadAuthority), codec(), keys());
    let ctx = context(f.actor(actor));
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        2,
        2,
        image::Rgba([60, 120, 180, 255]),
    ))
    .write_to(&mut out, ImageFormat::Png)?;
    let bytes = out.into_inner();
    let key = Uuid::new_v4();
    let id = PgMedia::initialized_id(ctx, key);
    media
        .initialize(
            ctx,
            key,
            InitUpload {
                purpose,
                mime: Mime::Png,
                byte_length: bytes.len(),
                sha256: Sha256::digest(&bytes).into(),
            },
        )
        .await?;
    let put = media.put(ctx, Uuid::new_v4(), id, Mime::Png, bytes).await?;
    media
        .finalize(ctx, Uuid::new_v4(), id, put.reference.unwrap().revision)
        .await?;
    Ok(id)
}
pub fn create(f: &Fixture, id: Option<Uuid>) -> PhotoMutation {
    PhotoMutation::Create(CreatePhotoBeacon {
        meeting: PhotoDescription::from_domain(f.meeting.clone(), id),
        physical_presence_acknowledged: true,
        photo_permission_acknowledged: id.is_some(),
    })
}
pub fn update(f: &Fixture, id: Option<Uuid>, revision: i64) -> PhotoMutation {
    PhotoMutation::Update(UpdatePhotoBeacon {
        meeting: PhotoDescription::from_domain(f.meeting.clone(), id),
        expected_revision: revision,
        physical_presence_acknowledged: true,
        photo_permission_acknowledged: id.is_some(),
    })
}
pub fn frame(f: &Fixture, beacon: Uuid, key: Uuid, m: &PhotoMutation) -> PhotoFrame {
    let raw = match m {
        PhotoMutation::Create(v) => serde_json::to_vec(v).unwrap(),
        PhotoMutation::Update(v) => serde_json::to_vec(v).unwrap(),
        _ => Vec::new(),
    };
    PhotoFrame::mutation(f.actor(f.owner), beacon, key, m, &raw).unwrap()
}
pub fn read(f: &Fixture, beacon: Uuid, actor: Uuid) -> PhotoFrame {
    PhotoFrame::read(f.actor(actor), beacon, None).unwrap()
}
pub fn content(f: &Fixture, beacon: Uuid, id: Uuid, actor: Uuid) -> PhotoFrame {
    PhotoFrame::read(f.actor(actor), beacon, Some((id, true))).unwrap()
}
pub async fn state(pool: &PgPool, id: Uuid) -> anyhow::Result<(String, Option<Uuid>, bool)> {
    Ok(sqlx::query_as(
        "select state,target_id,sanitized is null from concert_media where media_id=$1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?)
}

pub async fn pending(pool: &PgPool, f: &Fixture) -> anyhow::Result<Uuid> {
    let media = PgMedia::new(pool.clone(), Arc::new(UploadAuthority), codec(), keys());
    let ctx = context(f.actor(f.owner));
    let key = Uuid::new_v4();
    let id = PgMedia::initialized_id(ctx, key);
    media
        .initialize(
            ctx,
            key,
            InitUpload {
                purpose: Purpose::Beacon,
                mime: Mime::Png,
                byte_length: 1,
                sha256: [0; 32],
            },
        )
        .await?;
    Ok(id)
}
