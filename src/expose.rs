//! Expose cluster via NodePort and get connection host from kubeconfig.

use crate::service::ServiceType;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const NAMESPACE: &str = "default";

/// Get cluster server host from kubeconfig (current context).
/// Returns host without scheme/port, e.g. "api.cluster.example.com" or "1.2.3.4".
pub fn server_host_from_kubeconfig(kubectl: &Path, kubeconfig: &Path) -> Result<String, String> {
    let output = Command::new(kubectl)
        .arg("--kubeconfig")
        .arg(kubeconfig)
        .args([
            "config",
            "view",
            "--minify",
            "-o",
            "jsonpath={.clusters[0].cluster.server}",
        ])
        .output()
        .map_err(|e| format!("kubectl config view: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("kubectl config view failed: {stderr}"));
    }

    let url = String::from_utf8(output.stdout)
        .map_err(|e| format!("kubectl output utf-8: {e}"))?
        .trim()
        .to_string();

    parse_url_host(&url).ok_or_else(|| format!("could not parse server URL: {url}"))
}

fn parse_url_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?.split(':').next()?;
    if host.is_empty() {
        return None;
    }
    Some(host.to_string())
}

/// Create our own NodePort service (KubeBlocks-owned svc is reverted if patched). Return nodePort.
fn ensure_external_nodeport_service(
    kubectl: &Path,
    service: ServiceType,
    cluster_name: &str,
    kubeconfig: &Path,
) -> Result<u16, String> {
    let port = service.default_port();
    let component = service.kbcli_name();
    let port_name = service.port_name();
    let external_svc = format!("{cluster_name}-{component}-external");

    let exists = Command::new(kubectl)
        .arg("--kubeconfig")
        .arg(kubeconfig)
        .args(["get", "svc", &external_svc, "-n", NAMESPACE, "-o", "name"])
        .output()
        .map_err(|e| format!("kubectl get svc: {e}"))?;

    if !exists.status.success()
        || !String::from_utf8_lossy(&exists.stdout).trim().contains("service/")
    {
        let yaml = format!(
            r#"apiVersion: v1
kind: Service
metadata:
  name: {external_svc}
  namespace: {NAMESPACE}
spec:
  type: NodePort
  selector:
    app.kubernetes.io/instance: "{cluster_name}"
    apps.kubeblocks.io/component-name: {component}
    kubeblocks.io/role: primary
  ports:
  - port: {port}
    targetPort: {port}
    protocol: TCP
    name: {port_name}
"#
        );

        let mut apply = Command::new(kubectl)
            .arg("--kubeconfig")
            .arg(kubeconfig)
            .args(["apply", "-f", "-"])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("kubectl apply: {e}"))?;

        if let Some(mut stdin) = apply.stdin.take() {
            stdin.write_all(yaml.as_bytes()).map_err(|e| format!("stdin: {e}"))?;
        }
        let status = apply.wait().map_err(|e| format!("kubectl apply wait: {e}"))?;
        if !status.success() {
            return Err("kubectl apply -f - failed".to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(800));
    }

    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        for jsonpath in [
            &format!("{{.spec.ports[?(@.port=={port})].nodePort}}"),
            "{.spec.ports[*].nodePort}",
            "{.spec.ports[0].nodePort}",
        ] {
            let port_out = Command::new(kubectl)
                .arg("--kubeconfig")
                .arg(kubeconfig)
                .args([
                    "get", "svc", &external_svc, "-n", NAMESPACE,
                    "-o", &format!("jsonpath={jsonpath}"),
                ])
                .output()
                .map_err(|e| format!("kubectl get svc: {e}"))?;

            if !port_out.status.success() {
                continue;
            }
            let out = String::from_utf8_lossy(&port_out.stdout).trim().to_string();
            for port_str in out.split_whitespace() {
                if let Ok(p) = port_str.parse::<u16>() {
                    if p != 0 {
                        return Ok(p);
                    }
                }
            }
        }
    }

    Err(format!(
        "nodePort not assigned for service {external_svc}. Run: kubectl get svc {external_svc} -n {NAMESPACE} -o yaml"
    ))
}

/// Ensure NodePort is available (our external service) and return the port.
pub fn ensure_nodeport_and_get_port(
    kubectl: &Path,
    service: ServiceType,
    cluster_name: &str,
    kubeconfig: &Path,
) -> Result<u16, String> {
    ensure_external_nodeport_service(kubectl, service, cluster_name, kubeconfig)
}
