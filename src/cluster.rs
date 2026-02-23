//! Create/delete/list clusters via kbcli.

use crate::service::ServiceType;
use nanospinner::Spinner;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const POLL_INTERVAL_SECS: u64 = 3;
const TIMEOUT_SECS: u64 = 300; // 5 minutes

/// Parse storage/memory for kbcli: "2Gi" or "0.8Gi" -> number string; unit is Gi.
fn kbcli_quantity(s: &str) -> Result<String, String> {
    let s = s.trim();
    let num_str = s
        .strip_suffix("Gi")
        .or_else(|| s.strip_suffix("gi"))
        .unwrap_or(s);
    let num: f64 = num_str
        .trim()
        .parse()
        .map_err(|_| format!("invalid quantity: {s} (expected number or e.g. 2Gi)"))?;
    Ok(num.to_string())
}

/// Run kbcli cluster create <service> <name> with config.
pub fn create_cluster(
    kbcli: &Path,
    service: ServiceType,
    name: &str,
    kubeconfig: &Path,
    replicas: u32,
    storage: &str,
    cpu: &str,
    memory: &str,
) -> Result<(), String> {
    let storage_num = kbcli_quantity(storage)?;
    let memory_num = kbcli_quantity(memory)?;
    let output = Command::new(kbcli)
        .arg("--kubeconfig")
        .arg(kubeconfig)
        .args([
            "cluster",
            "create",
            service.kbcli_name(),
            name,
            "--replicas",
            &replicas.to_string(),
            "--storage",
            &storage_num,
            "--cpu",
            cpu,
            "--memory",
            &memory_num,
        ])
        .output()
        .map_err(|e| format!("kbcli failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("kbcli cluster create failed: {stderr}"));
    }
    Ok(())
}

/// Poll kbcli cluster list until status is Running or timeout.
pub fn wait_until_running(kbcli: &Path, name: &str, kubeconfig: &Path) -> Result<(), String> {
    let spinner = Spinner::new("Waiting for cluster to be Running...").start();
    let start = std::time::Instant::now();

    loop {
        if start.elapsed().as_secs() >= TIMEOUT_SECS {
            spinner.fail_with("Timeout waiting for cluster");
            return Err("cluster did not become Running within 5 minutes".to_string());
        }

        let output = match Command::new(kbcli)
            .arg("--kubeconfig")
            .arg(kubeconfig)
            .args(["cluster", "list", name])
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                spinner.fail_with("kbcli list failed");
                return Err(format!("kbcli cluster list failed: {e}"));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        if parse_status(&stdout) == Some("Running") {
            spinner.success();
            return Ok(());
        }

        std::thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
    }
}

/// Parse STATUS column from kbcli cluster list output (whitespace-separated, 5th column).
fn parse_status(stdout: &str) -> Option<&str> {
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.len() < 2 {
        return None;
    }
    let data_line = lines.get(1)?;
    let cols: Vec<&str> = data_line.split_whitespace().collect();
    cols.get(4).copied()
}

/// Delete cluster via kbcli cluster delete. If yes is false, prompt for confirmation.
/// Also removes fdb-created external NodePort services for this cluster name.
pub fn delete_cluster(
    kbcli: &Path,
    kubectl: &Path,
    name: &str,
    kubeconfig: &Path,
    yes: bool,
) -> Result<(), String> {
    if !yes {
        print!("Delete cluster \"{name}\"? [y/N]: ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|e| format!("read stdin: {e}"))?;
        let trimmed = line.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            return Err("aborted".to_string());
        }
    }

    let mut args = vec!["cluster", "delete", name];
    if yes {
        args.push("--auto-approve");
    }
    let output = Command::new(kbcli)
        .arg("--kubeconfig")
        .arg(kubeconfig)
        .args(args)
        .output()
        .map_err(|e| format!("kbcli failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("kbcli cluster delete failed: {stderr}"));
    }

    // Remove our external NodePort services if they exist.
    const NAMESPACE: &str = "default";
    for suffix in ["postgresql", "redis", "rabbitmq", "qdrant"] {
        let svc = format!("{name}-{suffix}-external");
        let _ = Command::new(kubectl)
            .arg("--kubeconfig")
            .arg(kubeconfig)
            .args(["delete", "svc", &svc, "-n", NAMESPACE, "--ignore-not-found=true"])
            .output();
    }
    Ok(())
}

/// List clusters via kbcli cluster list; parse and print name, type, status.
pub fn list_clusters(kbcli: &Path, kubeconfig: &Path) -> Result<(), String> {
    let output = Command::new(kbcli)
        .arg("--kubeconfig")
        .arg(kubeconfig)
        .args(["cluster", "list"])
        .output()
        .map_err(|e| format!("kbcli cluster list failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("kbcli cluster list failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.is_empty() {
        println!("No clusters found.");
        return Ok(());
    }
    // Pass through kbcli table as-is for consistency with kbcli output format.
    for line in lines {
        println!("{line}");
    }
    Ok(())
}
