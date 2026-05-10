use std::{
    fs,
    io::{self, Cursor, Read},
    path::Path,
    time::Duration,
};

use zip::ZipArchive;

/// URL of the GitHub Release asset that bundles the AV-sensitive payload
/// (kdmapper.exe + driver_standalone.sys). The `latest` tag automatically
/// resolves to the newest published release, so we don't need to bake in
/// version numbers.
pub const ASSETS_URL: &str =
    "https://github.com/hitolox/holiton-loader-cs2/releases/latest/download/assets.zip";

/// ZIP password — its only job is to bypass on-the-wire and on-disk antivirus
/// scanning of the archive contents. This is not a security boundary.
pub const ASSETS_PASSWORD: &str = "holiton";

/// Files contained in `assets.zip`. The loader expects these names verbatim.
pub const BUNDLED_FILES: &[&str] = &["kdmapper.exe", "driver_standalone.sys"];

/// Downloads and extracts `assets.zip` into `dir`, overwriting existing files.
/// All bundled members listed in [`BUNDLED_FILES`] must be present in the archive.
pub fn download_and_extract(dir: &Path) -> Result<(), String> {
    let bytes = http_get(ASSETS_URL)
        .map_err(|e| format!("Failed to download {}\n\n{}", ASSETS_URL, e))?;

    extract_zip(&bytes, dir).map_err(|e| {
        format!(
            "Downloaded the archive ({} bytes) but extraction failed.\n\n{}",
            bytes.len(),
            e
        )
    })?;

    let missing: Vec<&str> = BUNDLED_FILES
        .iter()
        .copied()
        .filter(|name| !dir.join(name).exists())
        .collect();

    if !missing.is_empty() {
        return Err(format!(
            "Extraction completed but the following files were not produced:\n  - {}",
            missing.join("\n  - ")
        ));
    }

    Ok(())
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .timeout_write(Duration::from_secs(60))
        .user_agent("holiton-loader/1.0 (+https://github.com/hitolox/holiton-loader-cs2)")
        .build();

    let response = agent.get(url).call().map_err(|e| match e {
        ureq::Error::Status(code, resp) => format!(
            "HTTP {} from {}: {}",
            code,
            resp.get_url(),
            resp.status_text()
        ),
        ureq::Error::Transport(t) => format!("transport error: {}", t),
    })?;

    let mut buf = Vec::with_capacity(2 * 1024 * 1024);
    response
        .into_reader()
        .take(50 * 1024 * 1024) // hard cap: 50 MiB
        .read_to_end(&mut buf)
        .map_err(|e| format!("read body: {}", e))?;

    Ok(buf)
}

fn extract_zip(bytes: &[u8], dir: &Path) -> io::Result<()> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("not a ZIP: {}", e)))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index_decrypt(i, ASSETS_PASSWORD.as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("entry {}: {}", i, e)))?
            .map_err(|_| io::Error::new(io::ErrorKind::PermissionDenied, "wrong ZIP password"))?;

        let Some(name) = entry.enclosed_name().map(Path::to_owned) else {
            continue;
        };
        let Some(file_name) = name.file_name() else {
            continue;
        };

        // Flatten — ignore any subdirectories inside the archive.
        let out_path = dir.join(file_name);
        let mut out = fs::File::create(&out_path)?;
        io::copy(&mut entry, &mut out)?;
    }

    Ok(())
}
