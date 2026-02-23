//! Extract account password from Kubernetes secret for a cluster.

use crate::service::ServiceType;
use std::path::Path;
use std::process::{Command, Stdio};

const NAMESPACE: &str = "default";

/// Get account password for cluster. Returns None for services without password (e.g. Qdrant).
pub fn get_password(
    kubectl: &Path,
    service: ServiceType,
    cluster_name: &str,
    kubeconfig: &Path,
) -> Result<Option<String>, String> {
    if !service.has_password() {
        return Ok(None);
    }

    let secret_name = service.secret_name(cluster_name);

    let mut kubectl_cmd = Command::new(kubectl)
        .args([
            "get",
            "secret",
            &secret_name,
            "-n",
            NAMESPACE,
            "-o",
            "jsonpath={.data.password}",
        ])
        .arg("--kubeconfig")
        .arg(kubeconfig)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("kubectl failed: {e}"))?;

    let kubectl_stdout = kubectl_cmd
        .stdout
        .take()
        .ok_or("kubectl stdout not captured")?;

    let output = Command::new("base64")
        .arg("-d")
        .stdin(kubectl_stdout)
        .output()
        .map_err(|e| format!("base64 -d failed: {e}"))?;

    let _ = kubectl_cmd.wait();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("base64 decode failed: {stderr}"));
    }

    let password = String::from_utf8(output.stdout).map_err(|e| format!("password not utf-8: {e}"))?;
    Ok(Some(password))
}
