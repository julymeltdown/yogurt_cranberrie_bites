use crate::dto::profile_dto::ProfileSearchPageDto;
use crate::errors::ProfileResult as Result;
use crate::ports::ProfileRepositoryPort;

#[derive(Clone)]
pub struct SearchProfilesUseCase<R> {
    repository: R,
}

impl<R> SearchProfilesUseCase<R>
where
    R: ProfileRepositoryPort,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<ProfileSearchPageDto> {
        // An empty or very short term matches a large fraction of the table.
        // Returning an empty page keeps the existing, non-erroring contract for
        // these clients while still avoiding the scan, and the same bound is
        // enforced for post search.
        let needle = match search_policy::normalize_search_term(query) {
            Ok(term) => term.to_lowercase(),
            Err(_) => {
                return Ok(ProfileSearchPageDto {
                    profiles: Vec::new(),
                    next_cursor: None,
                })
            }
        };
        if needle.is_empty() {
            return Ok(ProfileSearchPageDto {
                profiles: Vec::new(),
                next_cursor: None,
            });
        }

        let limit = if limit > 0 { limit.min(100) } else { 10 };
        let (profiles, next_cursor) = self.repository.search(&needle, cursor, limit).await?;
        Ok(ProfileSearchPageDto {
            profiles: profiles.into_iter().map(Into::into).collect(),
            next_cursor,
        })
    }
}
