//! Local owning-operator leaf. Never mount this producer as a browser endpoint.
//! New implementation against 0019, 0058 and additive 0063; no legacy rendering.
mod pack;
mod provision;
pub use pack::{load_pack, validate_pack, PackBytes, ValidatedPack};
pub use provision::{provision, InitialState, OperatorSelection, ProvisionedSeed};

#[derive(Debug)]
pub enum SeedError {
    InvalidPack(&'static str),
    InvalidSelection,
    EventUnavailable,
    NotPristine,
    Storage(sqlx::Error),
    Io(std::io::Error),
}
impl std::fmt::Display for SeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPack(reason) => write!(f, "Invalid mural seed pack: {reason}"),
            Self::InvalidSelection => f.write_str("Explicit owning-operator selection required"),
            Self::EventUnavailable => f.write_str("Canonical event unavailable"),
            Self::NotPristine => f.write_str("Mural is not the exact pristine selected seed"),
            Self::Storage(_) => f.write_str("Mural seed storage unavailable"),
            Self::Io(_) => f.write_str("Mural seed source unavailable"),
        }
    }
}
impl std::error::Error for SeedError {}
impl From<sqlx::Error> for SeedError {
    fn from(e: sqlx::Error) -> Self {
        Self::Storage(e)
    }
}
impl From<std::io::Error> for SeedError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
