/// A profile is unavailable to this viewer and should be concealed as not found.
///
/// Keeping this policy outcome typed lets the HTTP boundary return a stable 404
/// without guessing from arbitrary downstream error text.
#[derive(Debug, thiserror::Error)]
#[error("Profile not found")]
pub struct ProfileNotVisible;
