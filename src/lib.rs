//! ukyo_clicker core library
pub mod protocol;

/// Return crate name and version as a single String
pub fn banner() -> String {
    format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_version() {
        assert!(banner().contains(env!("CARGO_PKG_VERSION")));
    }
}
