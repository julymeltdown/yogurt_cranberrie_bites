mod support {
    use chrono::{DateTime, TimeZone, Utc};
    use post_service::domain::comment::Comment;
    use post_service::domain::post::Post;
    use uuid::Uuid;

    pub fn uuid(value: &str) -> Uuid {
        Uuid::parse_str(value).expect("valid uuid")
    }

    pub fn instant(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(seconds, 0)
            .single()
            .expect("valid instant")
    }

    pub fn post(id: Uuid, author_id: Uuid, content: &str) -> Post {
        Post {
            id,
            author_id,
            content: content.to_owned(),
            visibility: "PUBLIC".to_owned(),
            created_at: instant(1_700_000_000),
            comment_count: 0,
            like_count: 0,
        }
    }

    pub fn comment(id: Uuid, post_id: Uuid, author_id: Uuid, content: &str) -> Comment {
        Comment {
            id,
            post_id,
            author_id,
            content: content.to_owned(),
            created_at: instant(1_700_000_100),
        }
    }
}

#[path = "unit/dto_mapping.rs"]
mod dto_mapping;
