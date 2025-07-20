//! Product configuration only. The shared crate owns update transport and installation.
pub use rust_auto_updator::{Event, Release as Available, Service, handoff};

pub fn config() -> Result<rust_auto_updator::Config, String> {
    let mut config = rust_auto_updator::Config::new(
        "siarheisinelnikau",
        "folder-icons",
        crate::VERSION,
        "FolderIcons",
    )?;
    config.max_executable_size = 150 * 1024 * 1024;
    config.checksum_name = "{executable}.sha256".into();
    config.allow_asset_digest = true;
    config.install.stage_prefix = ".FolderIcons-update-".into();
    config.install.helper_name = "FolderIcons-helper.exe".into();
    Ok(config)
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_release_selection_keeps_existing_checksum_contract() {
        let mut config = super::config().unwrap();
        config.current_version = "0.9.0".parse().unwrap();
        let base = config.repository_url();
        let name = "FolderIcons-v0.10.0.exe";
        let data = serde_json::json!({"tag_name":"v0.10.0", "html_url":format!("{base}/releases/tag/v0.10.0"),
        "draft":false,"prerelease":false,"assets":[
            {"name":name,"size":2048,"browser_download_url":format!("{base}/releases/download/v0.10.0/{name}")},
            {"name":format!("{name}.sha256"),"size":96,"browser_download_url":format!("{base}/releases/download/v0.10.0/{name}.sha256")}
        ]});
        let release = rust_auto_updator::parse_release(&data.to_string(), &config)
            .unwrap()
            .unwrap();
        assert_eq!(release.version.to_string(), "0.10.0");
        assert!(release.checksum_url().unwrap().ends_with(".exe.sha256"));
        let mut foreign = data;
        foreign["assets"][0]["browser_download_url"] = "https://example.com/update.exe".into();
        assert!(rust_auto_updator::parse_release(&foreign.to_string(), &config).is_err());
    }
}
