use self_update::backends::github::Update;

pub fn check_for_updates() -> Result<String, Box<dyn std::error::Error>> {
    let status = Update::configure()
        .repo_owner("acer51-doctom")
        .repo_name("CM-version-manager")
        .bin_name("cm-manager") // Ensure this matches the compiled binary name in your GitHub releases
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()?
        .update()?;

    if status.updated() {
        Ok(format!("Updated successfully to version: {}! Please restart the app.", status.version()))
    } else {
        Ok("App is already up to date.".to_string())
    }
}