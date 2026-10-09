//! ukyo_clicker core library

/// Return crate name and version as a single String
pub fn banner() -> String {
  format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}
