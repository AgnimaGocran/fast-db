//! Resolve and optionally download kubectl and kbcli to ~/.fdb/bin.

use nanospinner::Spinner;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const KUBECTL_STABLE_URL: &str = "https://dl.k8s.io/release/stable.txt";
const GITHUB_LATEST_API: &str = "https://api.github.com/repos/apecloud/kbcli/releases/latest";

/// Directory for fdb-managed binaries: $FDB_HOME/bin or $HOME/.fdb/bin.
pub fn fdb_bin_dir() -> PathBuf {
    if let Ok(home) = std::env::var("FDB_HOME") {
        return PathBuf::from(home).join("bin");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".fdb").join("bin")
}

/// Look for executable in PATH, then in ~/.fdb/bin.
fn resolve_tool(name: &str) -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&paths) {
            let full = p.join(name);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    let bin = fdb_bin_dir().join(name);
    if bin.is_file() {
        Some(bin)
    } else {
        None
    }
}

pub fn resolve_kubectl() -> Result<PathBuf, String> {
    resolve_tool("kubectl").ok_or_else(|| "kubectl not found in PATH or ~/.fdb/bin".to_string())
}

pub fn resolve_kbcli() -> Result<PathBuf, String> {
    resolve_tool("kbcli").ok_or_else(|| "kbcli not found in PATH or ~/.fdb/bin".to_string())
}

/// Ensure kubectl and kbcli exist; download to ~/.fdb/bin if missing.
pub fn ensure_tools() -> Result<(), String> {
    let need_kubectl = resolve_tool("kubectl").is_none();
    let need_kbcli = resolve_tool("kbcli").is_none();
    if !need_kubectl && !need_kbcli {
        return Ok(());
    }
    let bin_dir = fdb_bin_dir();
    fs::create_dir_all(&bin_dir).map_err(|e| format!("create {:?}: {e}", bin_dir))?;

    if need_kubectl {
        download_kubectl(&bin_dir)?;
    }
    if need_kbcli {
        download_kbcli(&bin_dir)?;
    }
    Ok(())
}

fn download_with_progress(
    url: &str,
    dest_path: &Path,
    name: &str,
    total_bytes: Option<u64>,
) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;

    let total = total_bytes.or_else(|| {
        response
            .header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok())
    });

    let mut reader = response.into_reader();
    let mut file = fs::File::create(dest_path).map_err(|e| format!("create file: {e}"))?;
    let mut buf = [0u8; 65536];
    let mut downloaded: u64 = 0;
    let spinner = Spinner::new("").start();

    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| format!("write: {e}"))?;
        downloaded += n as u64;
        let msg = if let Some(t) = total {
            let pct = (100 * downloaded) / t;
            format!("Downloading {name} {} MiB / {} MiB ({}%)", downloaded / 1024 / 1024, t / 1024 / 1024, pct)
        } else {
            format!("Downloading {name} {} MiB", downloaded / 1024 / 1024)
        };
        spinner.update(&msg);
    }
    spinner.success_with(&format!("Downloaded {name}"));
    drop(file);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dest_path).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dest_path, perms).map_err(|e| format!("chmod: {e}"))?;
    }
    Ok(())
}

fn download_kubectl(bin_dir: &Path) -> Result<(), String> {
    let version: String = ureq::get(KUBECTL_STABLE_URL)
        .call()
        .map_err(|e| format!("GET stable.txt: {e}"))?
        .into_string()
        .map_err(|e| format!("stable.txt utf-8: {e}"))?
        .trim()
        .to_string();

    let (os, arch) = target_os_arch();
    let url = format!(
        "https://dl.k8s.io/release/{version}/bin/{os}/{arch}/kubectl"
    );
    let dest = bin_dir.join("kubectl");
    download_with_progress(&url, &dest, "kubectl", None)?;
    Ok(())
}

fn target_os_arch() -> (String, String) {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let os = match os {
        "linux" => "linux",
        "macos" => "darwin",
        "windows" => "windows",
        _ => os,
    };
    let arch = match arch {
        "x86_64" => "amd64",
        "aarch64" | "arm64" => "arm64",
        _ => arch,
    };
    (os.to_string(), arch.to_string())
}

fn download_kbcli(bin_dir: &Path) -> Result<(), String> {
    let api_response = ureq::get(GITHUB_LATEST_API)
        .set("Accept", "application/vnd.github.v3+json")
        .set("User-Agent", "fdb-cli")
        .call()
        .map_err(|e| format!("GET GitHub API: {e}"))?
        .into_string()
        .map_err(|e| format!("GitHub API utf-8: {e}"))?;

    let tag = parse_tag_name(&api_response).ok_or("could not parse tag_name from GitHub API")?;

    let (os, arch) = target_os_arch();
    let archive_name = format!("kbcli-{os}-{arch}-{tag}.tar.gz");
    let url = format!(
        "https://github.com/apecloud/kbcli/releases/download/{tag}/{archive_name}"
    );

    let temp_tar = bin_dir.join("kbcli-download.tar.gz");
    download_with_progress(&url, &temp_tar, "kbcli", None)?;

    extract_kbcli_from_tar_gz(&temp_tar, bin_dir)?;
    let _ = fs::remove_file(&temp_tar);
    Ok(())
}

fn parse_tag_name(json: &str) -> Option<String> {
    let needle = "\"tag_name\":\"";
    let start = json.find(needle)? + needle.len();
    let end = json[start..].find('"')?;
    Some(json[start..start + end].to_string())
}

fn extract_kbcli_from_tar_gz(tar_gz_path: &Path, bin_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(tar_gz_path).map_err(|e| format!("open archive: {e}"))?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);

    for entry in archive.entries().map_err(|e| format!("tar entries: {e}"))? {
        let mut entry = entry.map_err(|e| format!("tar entry: {e}"))?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "kbcli" || name.starts_with("kbcli") {
            entry.unpack_in(bin_dir).map_err(|e| format!("unpack: {e}"))?;
            let extracted = bin_dir.join(&path);
            let dest = bin_dir.join("kbcli");
            if extracted != dest {
                if dest.exists() {
                    let _ = fs::remove_file(&dest);
                }
                if fs::rename(&extracted, &dest).is_err() {
                    fs::copy(&extracted, &dest).map_err(|e| format!("copy: {e}"))?;
                    let _ = fs::remove_file(&extracted);
                }
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let dest = bin_dir.join("kbcli");
                if let Ok(m) = fs::metadata(&dest) {
                    let mut p = m.permissions();
                    p.set_mode(0o755);
                    let _ = fs::set_permissions(&dest, p);
                }
            }
            return Ok(());
        }
    }
    Err("kbcli binary not found inside archive".to_string())
}
