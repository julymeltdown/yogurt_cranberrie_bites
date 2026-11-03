use async_trait::async_trait;
use platform::cpu::{AdmissionClass, WorkerPool};
use profile_service::adapters::out::media::image_avatar_processor::ImageAvatarProcessor;
use profile_service::application::dto::profile_dto::AvatarDto;
use profile_service::application::errors::ProfileResult as Result;
use profile_service::application::ports::{AvatarProcessorPort, AvatarRepositoryPort};
use profile_service::application::use_cases::get_avatar::GetAvatarUseCase;
use profile_service::application::use_cases::upload_avatar::UploadAvatarUseCase;
use profile_service::domain::avatar::{Avatar, MAX_AVATAR_BYTES, MAX_AVATAR_PIXELS};
use proptest::prelude::*;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::oneshot;
use uuid::Uuid;

const USER_ID: &str = "11111111-1111-1111-1111-111111111111";

fn png_header_with_dimensions(width: u32, height: u32) -> Vec<u8> {
    let mut png = crate::support::png_fixture();
    png[16..20].copy_from_slice(&width.to_be_bytes());
    png[20..24].copy_from_slice(&height.to_be_bytes());
    let checksum = png_crc32(&png[12..29]);
    png[29..33].copy_from_slice(&checksum.to_be_bytes());
    png
}

fn png_crc32(bytes: &[u8]) -> u32 {
    let mut checksum = u32::MAX;
    for byte in bytes {
        checksum ^= u32::from(*byte);
        for _ in 0..8 {
            checksum = if checksum & 1 == 1 {
                (checksum >> 1) ^ 0xedb8_8320
            } else {
                checksum >> 1
            };
        }
    }
    !checksum
}

fn property_processor() -> &'static ImageAvatarProcessor {
    static PROCESSOR: OnceLock<ImageAvatarProcessor> = OnceLock::new();
    PROCESSOR.get_or_init(|| {
        let pool = WorkerPool::new(2).expect("avatar worker pool");
        ImageAvatarProcessor::new(pool.class(4).expect("avatar decode admission"))
    })
}

fn property_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("avatar property-test runtime")
    })
}

fn process_for_property_test(
    content: Vec<u8>,
    content_type: &str,
    filename: &str,
) -> Result<AvatarDto> {
    property_runtime().block_on(property_processor().process(content, content_type, filename))
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn arbitrary_bounded_avatar_payloads_never_panic(input in prop::collection::vec(any::<u8>(), 0..=4096), format in 0usize..4) {
        let content_type = ["image/png", "image/jpeg", "image/webp", "image/gif"][format];
        if let Ok(avatar) = process_for_property_test(input, content_type, "fuzzed.bin") {
            prop_assert!(!avatar.content.is_empty());
            prop_assert!(avatar.content.len() <= MAX_AVATAR_BYTES);
            prop_assert_eq!(avatar.content_type, content_type);
        }
    }

    #[test]
    fn mutations_of_valid_avatar_seeds_remain_bounded(source in 0usize..3, edits in prop::collection::vec((0usize..2048, any::<u8>()), 0..=8), truncation in 0usize..=8, suffix in prop::collection::vec(any::<u8>(), 0..=8)) {
        let (mut input, content_type) = match source {
            0 => (crate::support::png_fixture(), "image/png"),
            1 => (crate::support::jpeg_fixture(), "image/jpeg"),
            _ => (crate::support::webp_fixture(), "image/webp"),
        };
        for (offset, replacement) in edits {
            if !input.is_empty() {
                let index = offset % input.len();
                input[index] = replacement;
            }
        }
        input.truncate(input.len().saturating_sub(truncation));
        input.extend_from_slice(&suffix);

        if let Ok(avatar) = process_for_property_test(input, content_type, "mutated.bin") {
            prop_assert!(!avatar.content.is_empty());
            prop_assert!(avatar.content.len() <= MAX_AVATAR_BYTES);
            prop_assert_eq!(avatar.content_type, content_type);
            let reloaded = process_for_property_test(avatar.content, content_type, "canonical.bin")
                .expect("service-produced canonical bytes remain decodable");
            prop_assert_eq!(reloaded.content_type, content_type);
        }
    }

    #[test]
    fn oversized_png_dimensions_are_rejected_before_bitmap_decode(dimension in 4097u32..=8192, transpose in any::<bool>()) {
        let (width, height) = if transpose {
            (dimension, 4097)
        } else {
            (4097, dimension)
        };
        let error = process_for_property_test(
            png_header_with_dimensions(width, height),
            "image/png",
            "dimensions.png",
        )
            .expect_err("an oversized bitmap must be rejected from its header");
        prop_assert!(
            error.to_string().contains("exceeds the"),
            "unexpected dimension error: {error:#}"
        );
        prop_assert!(error.to_string().contains(&MAX_AVATAR_PIXELS.to_string()));
    }
}

#[derive(Clone, Default)]
struct InMemoryAvatarRepository {
    saved: Arc<Mutex<Vec<SavedAvatar>>>,
    find_calls: Arc<Mutex<Vec<Uuid>>>,
}

impl InMemoryAvatarRepository {
    fn saved_avatars(&self) -> Vec<SavedAvatar> {
        self.saved.lock().expect("avatar repository lock").clone()
    }

    fn find_calls(&self) -> Vec<Uuid> {
        self.find_calls
            .lock()
            .expect("avatar repository lock")
            .clone()
    }
}

fn new_use_case(
    repository: InMemoryAvatarRepository,
) -> UploadAvatarUseCase<InMemoryAvatarRepository, ImageAvatarProcessor> {
    let pool = WorkerPool::new(2).expect("avatar worker pool");
    let admission = pool.class(4).expect("avatar decode admission");
    UploadAvatarUseCase::new(repository, ImageAvatarProcessor::new(admission))
}

fn occupied_decoder(
    admission: AdmissionClass,
) -> (
    std::sync::mpsc::Sender<()>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
    oneshot::Receiver<()>,
) {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let task = tokio::spawn(async move {
        admission
            .run(move || {
                let _ = started_tx.send(());
                release_rx.recv()?;
                Ok(())
            })
            .await
    });
    (release_tx, task, started_rx)
}

#[derive(Clone)]
struct SavedAvatar {
    user_id: Uuid,
    avatar: AvatarDto,
}

#[async_trait]
impl AvatarRepositoryPort for InMemoryAvatarRepository {
    async fn save(&self, user_id: Uuid, avatar: &AvatarDto) -> Result<()> {
        self.saved
            .lock()
            .expect("avatar repository lock")
            .push(SavedAvatar {
                user_id,
                avatar: avatar.clone(),
            });
        Ok(())
    }

    async fn find(&self, user_id: Uuid) -> Result<Option<AvatarDto>> {
        self.find_calls
            .lock()
            .expect("avatar repository lock")
            .push(user_id);
        Ok(self
            .saved
            .lock()
            .expect("avatar repository lock")
            .iter()
            .rev()
            .find(|saved| saved.user_id == user_id)
            .map(|saved| saved.avatar.clone()))
    }
}

#[tokio::test]
async fn saves_png_avatar_and_returns_stable_owner_id() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    let response = use_case
        .execute(
            USER_ID,
            crate::support::png_fixture(),
            " IMAGE/PNG ",
            " avatar.png ",
        )
        .await
        .expect("avatar upload");

    assert_eq!(response.user_id, USER_ID);
    let saved = repository.saved_avatars();
    assert_eq!(saved.len(), 1);
    assert_eq!(
        saved[0].user_id,
        Uuid::parse_str(USER_ID).expect("valid uuid")
    );
    assert_eq!(saved[0].avatar.content, crate::support::png_fixture());
    assert_eq!(saved[0].avatar.content_type, "image/png");
    assert_eq!(saved[0].avatar.filename, "avatar.png");
}

#[tokio::test]
async fn rejects_svg_avatar_without_saving() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    let error = use_case
        .execute(USER_ID, b"<svg/>".to_vec(), "image/svg+xml", "avatar.svg")
        .await
        .expect_err("svg avatar rejected");

    assert!(error.to_string().contains("unsupported"));
    assert!(repository.saved_avatars().is_empty());
}

#[tokio::test]
async fn get_avatar_returns_saved_avatar_as_dto() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = GetAvatarUseCase::new(repository.clone());
    let user_id = Uuid::parse_str(USER_ID).expect("valid uuid");
    let avatar =
        Avatar::from_processed(crate::support::jpeg_fixture(), " IMAGE/JPEG ", " face.jpg ")
            .expect("avatar");
    repository
        .save(user_id, &avatar.clone().into())
        .await
        .expect("save avatar");

    let response = use_case
        .execute(&format!(" {USER_ID} "))
        .await
        .expect("get avatar")
        .expect("avatar exists");

    assert_eq!(response.content, crate::support::jpeg_fixture());
    assert_eq!(response.content_type, "image/jpeg");
    assert_eq!(response.filename, "face.jpg");
    assert_eq!(repository.find_calls(), vec![user_id]);
}

#[tokio::test]
async fn get_avatar_returns_none_when_avatar_is_missing() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = GetAvatarUseCase::new(repository.clone());
    let user_id = Uuid::parse_str(USER_ID).expect("valid uuid");

    let response = use_case.execute(USER_ID).await.expect("get avatar");

    assert_eq!(response, None);
    assert_eq!(repository.find_calls(), vec![user_id]);
}

#[tokio::test]
async fn get_avatar_rejects_blank_user_id_without_repository_lookup() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = GetAvatarUseCase::new(repository.clone());

    let error = use_case
        .execute("   ")
        .await
        .expect_err("blank user id rejected");

    assert_eq!(error.to_string(), "User ID is required");
    assert!(repository.find_calls().is_empty());
}

#[tokio::test]
async fn get_avatar_rejects_malformed_user_id_without_repository_lookup() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = GetAvatarUseCase::new(repository.clone());

    let error = use_case
        .execute("not-a-uuid")
        .await
        .expect_err("malformed user id rejected");

    assert_eq!(error.to_string(), "User ID is required");
    assert!(repository.find_calls().is_empty());
}

#[tokio::test]
async fn rejects_empty_content_type_without_saving() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    let error = use_case
        .execute(USER_ID, b"image".to_vec(), "  ", "avatar.png")
        .await
        .expect_err("empty content type rejected");

    assert!(error.to_string().contains("required"));
    assert!(repository.saved_avatars().is_empty());
}

#[tokio::test]
async fn rejects_unknown_content_type_without_saving() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    let error = use_case
        .execute(
            USER_ID,
            b"image".to_vec(),
            "application/octet-stream",
            "avatar.bin",
        )
        .await
        .expect_err("unknown content type rejected");

    assert!(error.to_string().contains("unsupported"));
    assert!(repository.saved_avatars().is_empty());
}

#[tokio::test]
async fn rejects_oversize_avatar_without_saving() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());
    let oversize_avatar = vec![0; 1_048_577];

    let error = use_case
        .execute(USER_ID, oversize_avatar, "image/png", "avatar.png")
        .await
        .expect_err("oversize avatar rejected");

    // The message is built from the limit constant, so it cannot claim a bound
    // that is no longer the one enforced.
    assert!(error.to_string().contains(&MAX_AVATAR_BYTES.to_string()));
    assert!(repository.saved_avatars().is_empty());
}

#[tokio::test]
async fn rejects_content_that_does_not_match_the_declared_image_type() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    // Active content declared as an image must be refused, and so must an image
    // of one format declared as another. Without this, arbitrary bytes could be
    // stored and later served under an image content type.
    for (content, declared) in [
        (b"<svg onload=alert(1)></svg>".to_vec(), "image/png"),
        (b"not an image at all".to_vec(), "image/jpeg"),
        (crate::support::png_fixture(), "image/jpeg"),
        (crate::support::jpeg_fixture(), "image/webp"),
    ] {
        let error = use_case
            .execute(USER_ID, content, declared, "avatar.bin")
            .await
            .expect_err("mismatched content must be rejected");
        assert!(
            error.to_string().contains("does not match"),
            "unexpected error for {declared}: {error}"
        );
    }
    assert!(repository.saved_avatars().is_empty());
}

#[tokio::test]
async fn accepts_each_supported_image_signature() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    for (content, declared, filename) in [
        (crate::support::png_fixture(), "image/png", "a.png"),
        (crate::support::jpeg_fixture(), "image/jpeg", "a.jpg"),
        (crate::support::webp_fixture(), "image/webp", "a.webp"),
        (crate::support::gif_fixture(), "image/gif", "a.gif"),
    ] {
        use_case
            .execute(USER_ID, content, declared, filename)
            .await
            .expect("valid image accepted");
    }
    assert_eq!(repository.saved_avatars().len(), 4);
}

#[tokio::test]
async fn rejects_a_polyglot_that_merely_starts_with_a_valid_signature() {
    // This is the case the container check could not catch and the reason the
    // decode exists. The payload begins with a byte-perfect PNG signature and
    // follows it with active content, which is what a signature test accepts and
    // a decoder must refuse.
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    let mut polyglot = crate::support::png_fixture();
    polyglot.extend_from_slice(b"<svg onload=alert(1)></svg>");
    // The signature is untouched, so only a real decode can address this.
    assert!(polyglot.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]));
    let appended = b"<svg onload=alert(1)></svg>";

    // A PNG with trailing data is still a valid PNG, so the guarantee is not
    // that the upload is refused but that the appended bytes do not survive into
    // what is stored and later served. That is what makes the re-encode load
    // bearing rather than decorative.
    use_case
        .execute(USER_ID, polyglot, "image/png", "avatar.png")
        .await
        .expect("a valid PNG carrying trailing data is accepted and cleaned");

    let saved = repository.saved_avatars();
    assert_eq!(saved.len(), 1);
    let stored = &saved[0].avatar.content;
    assert!(
        !stored
            .windows(appended.len())
            .any(|window| window == appended),
        "appended content must not survive into the stored avatar"
    );
    // What remains must still be the image that was uploaded.
    let decoded =
        image::load_from_memory_with_format(stored, image::ImageFormat::Png).expect("valid png");
    assert_eq!((decoded.width(), decoded.height()), (2, 2));
}

#[tokio::test]
async fn rejects_a_truncated_image() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    // Cut a valid PNG in the middle, keeping the signature intact.
    let truncated = crate::support::png_fixture()[..20].to_vec();
    assert!(truncated.starts_with(&[0x89, b'P', b'N', b'G']));

    let error = use_case
        .execute(USER_ID, truncated, "image/png", "avatar.png")
        .await
        .expect_err("a truncated image must be rejected");
    assert!(!error.to_string().is_empty());
    assert!(repository.saved_avatars().is_empty());
}

#[tokio::test]
async fn rejects_an_image_that_decodes_to_no_pixels() {
    // A well-formed container with no image data passes a signature check but has
    // nothing to decode, and must not reach storage as an "image".
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    let mut empty_png = Vec::new();
    empty_png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    empty_png.extend_from_slice(&[0x00, 0x00, 0x00, 0x0d]);
    empty_png.extend_from_slice(b"IHDR");
    empty_png.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    empty_png.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    empty_png.extend_from_slice(&[0x08, 0x06, 0x00, 0x00, 0x00]);
    empty_png.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    empty_png.extend_from_slice(b"IEND");
    empty_png.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);

    let error = use_case
        .execute(USER_ID, empty_png, "image/png", "avatar.png")
        .await
        .expect_err("an image with no pixels must be rejected");
    assert!(!error.to_string().is_empty());
    assert!(repository.saved_avatars().is_empty());
}

#[tokio::test]
async fn rejects_payloads_larger_than_the_documented_limit() {
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    let mut oversized = crate::support::png_fixture();
    oversized.resize(profile_service::domain::avatar::MAX_AVATAR_BYTES + 1, 0);

    let error = use_case
        .execute(USER_ID, oversized, "image/png", "avatar.png")
        .await
        .expect_err("an oversized payload must be rejected before decoding");
    assert!(
        error.to_string().contains(&MAX_AVATAR_BYTES.to_string()),
        "the error must name the limit, got: {error}"
    );
    assert!(repository.saved_avatars().is_empty());
}

#[tokio::test]
async fn a_stored_avatar_is_reencoded_rather_than_stored_verbatim() {
    // Re-encoding is what proves the stored bytes are produced by this service.
    // If the stored content were byte-identical to the upload, trailing data
    // appended to a valid image would survive into what is served.
    let repository = InMemoryAvatarRepository::default();
    let use_case = new_use_case(repository.clone());

    let uploaded = crate::support::png_fixture();
    use_case
        .execute(USER_ID, uploaded.clone(), "image/png", "avatar.png")
        .await
        .expect("valid image accepted");

    let saved = repository.saved_avatars();
    assert_eq!(saved.len(), 1);
    // It must still be a decodable PNG of the same dimensions...
    let decoded =
        image::load_from_memory_with_format(&saved[0].avatar.content, image::ImageFormat::Png)
            .expect("the stored avatar must still be a valid PNG");
    assert_eq!((decoded.width(), decoded.height()), (2, 2));
    // ...and the stored type is the declared one.
    assert_eq!(saved[0].avatar.content_type, "image/png");
}

#[tokio::test]
async fn avatar_decode_admission_bounds_running_and_queued_uploads() {
    let repository = InMemoryAvatarRepository::default();
    let pool = WorkerPool::new(1).expect("avatar worker pool");
    let admission = pool.class(1).expect("one queued decode");
    let processor = ImageAvatarProcessor::new(admission.clone());
    let use_case = UploadAvatarUseCase::new(repository.clone(), processor.clone());

    // Occupy the only decoder worker, then fill the one-request queue.
    let (release, running, started) = occupied_decoder(admission.clone());
    started.await.expect("decoder started");
    let queued_use_case = use_case.clone();
    let queued = tokio::spawn(async move {
        queued_use_case
            .execute(
                USER_ID,
                crate::support::png_fixture(),
                "image/png",
                "queued.png",
            )
            .await
    });
    while admission.stats().admitted < 2 {
        tokio::task::yield_now().await;
    }

    // A third upload is refused immediately; it does not grow an unbounded
    // queue or reach persistence.
    let error = use_case
        .execute(
            USER_ID,
            crate::support::png_fixture(),
            "image/png",
            "rejected.png",
        )
        .await
        .expect_err("decode admission must be bounded");
    assert_eq!(
        error,
        profile_service::application::errors::ProfileError::Busy
    );
    assert_eq!(processor.admission_stats().rejected, 1);
    assert!(repository.saved_avatars().is_empty());

    release.send(()).expect("release occupied decoder");
    running
        .await
        .expect("occupied task")
        .expect("occupied work");
    queued
        .await
        .expect("queued upload task")
        .expect("queued upload");
    assert_eq!(repository.saved_avatars().len(), 1);
}
