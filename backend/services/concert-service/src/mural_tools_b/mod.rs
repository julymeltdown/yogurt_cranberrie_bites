//! Wall B extends the incumbent PostgreSQL mural and the original canonical scope.
pub mod admin;
mod authority;
pub mod deletion;
pub mod protocol;
mod repository;
mod runtime;
mod witness;
pub use admin::MuralAdmin;
pub use authority::ScopedMuralAuthority;
pub use repository::PgMuralTools;
pub(crate) use repository::{complete_in, correlate_in, legacy, pencil_plan_in, record_in};
pub use runtime::MuralToolsRuntime;
pub use witness::MuralToolsNativeResolver;
