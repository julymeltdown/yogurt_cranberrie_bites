//! Wall B extends the incumbent PostgreSQL mural and the original canonical scope.
mod authority;
pub mod protocol;
mod repository;
mod runtime;
mod witness;
pub use authority::ScopedMuralAuthority;
pub use repository::PgMuralTools;
pub(crate) use repository::{complete_in, correlate_in, legacy, pencil_plan_in, record_in};
pub use runtime::MuralToolsRuntime;
pub use witness::MuralToolsNativeResolver;
