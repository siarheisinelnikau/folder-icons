pub mod engine;
pub mod i18n;
pub mod ini;
pub mod platform;
pub mod render;
pub mod selection;
pub mod settings;
pub mod updates;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPOSITORY: &str = "https://github.com/siarheisinelnikau/folder-icons";

pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
