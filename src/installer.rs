use std::fs::File;
use std::path::PathBuf;
use std::io::Cursor;
use std::sync::mpsc::Sender;
use zip::ZipArchive;

pub fn install_build(build_version: String, install_base_dir: String, tx: Sender<String>) {
    // 1. Determine the OS to download the correct artifact
    let os_suffix = if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    };

    // NOTE: You will need to replace this URL with the actual Jenkins API/Download URL.
    // I am using a placeholder structure based on standard Jenkins artifact URLs.
    let download_url = format!(
        "https://jenkins.chromapper.example.com/job/ChroMapper/{}/artifact/ChroMapper-{}.zip",
        build_version.replace("Build #", ""), // Cleans "Build #1205" to "1205"
        os_suffix
    );

    let target_dir = PathBuf::from(&install_base_dir)
        .join("versions")
        .join(&build_version.replace(" ", "_")); // e.g., Documents/cm-version-manager/versions/Build_#1205

    // Let the UI know we are starting
    let _ = tx.send(format!("Starting download for {}...", build_version));

    // 2. Download the ZIP file
    let response = match reqwest::blocking::get(&download_url) {
        Ok(res) => res,
        Err(e) => {
            let _ = tx.send(format!("Error downloading: {}", e));
            return;
        }
    };

    if !response.status().is_success() {
        let _ = tx.send(format!("Failed to find build on server (Status {})", response.status()));
        return;
    }

    let _ = tx.send("Download complete! Extracting files...".to_string());
    let zip_bytes = response.bytes().unwrap();

    // 3. Extract the ZIP file
    std::fs::create_dir_all(&target_dir).unwrap_or_default();
    let reader = Cursor::new(zip_bytes);
    let mut archive = match ZipArchive::new(reader) {
        Ok(a) => a,
        Err(_) => {
            let _ = tx.send("Error: Downloaded file is not a valid ZIP.".to_string());
            return;
        }
    };

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = match file.enclosed_name() {
            Some(path) => target_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath).unwrap();
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p).unwrap();
                }
            }
            let mut outfile = File::create(&outpath).unwrap();
            std::io::copy(&mut file, &mut outfile).unwrap();
            
            // On Linux/Mac, ensure the executable has run permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if outpath.extension().is_none() { // Assuming main executable has no extension on Mac/Linux
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(0o755)).unwrap();
                }
            }
        }
    }

    let _ = tx.send(format!("Success! {} installed to {:?}", build_version, target_dir));
}