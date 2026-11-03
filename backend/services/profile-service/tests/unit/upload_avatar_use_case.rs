use async_trait::async_trait;
use profile_service::application::dto::profile_dto::AvatarDto;
use profile_service::application::errors::{ProfileError, ProfileResult as Result};
use profile_service::application::ports::{AvatarProcessorPort, AvatarRepositoryPort};
use profile_service::application::use_cases::upload_avatar::UploadAvatarUseCase;
use profile_service::domain::avatar::{Avatar, MAX_AVATAR_BYTES};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const USER_ID: &str = "11111111-1111-1111-1111-111111111111";

#[derive(Clone, Default)]
struct InMemoryAvatarRepository {
    saved: Arc<Mutex<Vec<(Uuid, AvatarDto)>>>,
}

impl InMemoryAvatarRepository {
    fn saved(&self) -> Vec<(Uuid, AvatarDto)> {
        self.saved.lock().expect("avatar repository lock").clone()
    }
}

#[async_trait]
impl AvatarRepositoryPort for InMemoryAvatarRepository {
    async fn save(&self, user_id: Uuid, avatar: &AvatarDto) -> Result<()> {
        self.saved
            .lock()
            .expect("avatar repository lock")
            .push((user_id, avatar.clone()));
        Ok(())
    }

    async fn find(&self, _user_id: Uuid) -> Result<Option<AvatarDto>> {
        Ok(None)
    }
}

#[derive(Clone)]
struct RecordingAvatarProcessor {
    state: Arc<Mutex<ProcessorState>>,
}

struct ProcessorState {
    requests: Vec<ProcessRequest>,
    result: Avatar,
    failure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessRequest {
    content: Vec<u8>,
    content_type: String,
    filename: String,
}

impl RecordingAvatarProcessor {
    fn returning(result: Avatar) -> Self {
        Self {
            state: Arc::new(Mutex::new(ProcessorState {
                requests: Vec::new(),
                result,
                failure: false,
            })),
        }
    }

    fn failing(result: Avatar) -> Self {
        Self {
            state: Arc::new(Mutex::new(ProcessorState {
                requests: Vec::new(),
                result,
                failure: true,
            })),
        }
    }

    fn requests(&self) -> Vec<ProcessRequest> {
        self.state.lock().expect("processor lock").requests.clone()
    }
}

#[async_trait]
impl AvatarProcessorPort for RecordingAvatarProcessor {
    async fn process(
        &self,
        content: Vec<u8>,
        content_type: &str,
        filename: &str,
    ) -> Result<AvatarDto> {
        let mut state = self.state.lock().expect("processor lock");
        state.requests.push(ProcessRequest {
            content,
            content_type: content_type.to_owned(),
            filename: filename.to_owned(),
        });
        if state.failure {
            return Err(ProfileError::InvalidInput(
                "injected image-processor failure",
            ));
        }
        Ok(state.result.clone().into())
    }
}

fn processed_avatar() -> Avatar {
    Avatar::from_processed(b"canonical output".to_vec(), "image/png", "avatar.png")
        .expect("valid processed-value fixture")
}

#[tokio::test]
async fn delegates_processing_and_persists_the_port_result() {
    let repository = InMemoryAvatarRepository::default();
    let processor = RecordingAvatarProcessor::returning(processed_avatar());
    let use_case = UploadAvatarUseCase::new(repository.clone(), processor.clone());
    let input = b"untrusted multipart bytes".to_vec();

    let response = use_case
        .execute(USER_ID, input.clone(), " IMAGE/PNG ", " raw.png ")
        .await
        .expect("upload");

    let owner_id = Uuid::parse_str(USER_ID).expect("valid owner");
    assert_eq!(response.user_id, USER_ID);
    assert_eq!(
        processor.requests(),
        vec![ProcessRequest {
            content: input,
            content_type: " IMAGE/PNG ".into(),
            filename: " raw.png ".into(),
        }]
    );
    assert_eq!(
        repository.saved(),
        vec![(owner_id, processed_avatar().into())]
    );
}

#[tokio::test]
async fn processor_failure_does_not_persist_an_avatar() {
    let repository = InMemoryAvatarRepository::default();
    let processor = RecordingAvatarProcessor::failing(processed_avatar());
    let use_case = UploadAvatarUseCase::new(repository.clone(), processor.clone());

    let error = use_case
        .execute(USER_ID, b"bad image".to_vec(), "image/png", "avatar.png")
        .await
        .expect_err("processor failure");

    assert!(error
        .to_string()
        .contains("injected image-processor failure"));
    assert_eq!(processor.requests().len(), 1);
    assert!(repository.saved().is_empty());
}

#[tokio::test]
async fn invalid_owner_and_invalid_sizes_are_rejected_before_processing() {
    let repository = InMemoryAvatarRepository::default();
    let processor = RecordingAvatarProcessor::returning(processed_avatar());
    let use_case = UploadAvatarUseCase::new(repository.clone(), processor.clone());

    assert!(use_case
        .execute("not-a-uuid", b"input".to_vec(), "image/png", "avatar.png")
        .await
        .is_err());
    assert!(use_case
        .execute(USER_ID, Vec::new(), "image/png", "avatar.png")
        .await
        .is_err());
    assert!(use_case
        .execute(
            USER_ID,
            vec![0; MAX_AVATAR_BYTES + 1],
            "image/png",
            "avatar.png",
        )
        .await
        .is_err());

    assert!(processor.requests().is_empty());
    assert!(repository.saved().is_empty());
}
