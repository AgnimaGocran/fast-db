//! Background kubectl port-forward to expose PostgreSQL locally.

use std::path::Path;
use std::process::{Child, Command, Stdio};

const REMOTE_PORT: u16 = 5432;

/// Start `kubectl port-forward svc/<name>-postgresql :5432` in background.
/// Returns (child process, local port). Caller must not kill the child so port-forward stays alive.
pub fn start_port_forward(
    kubectl: &Path,
    cluster_name: &str,
    kubeconfig: &Path,
) -> Result<(Child, u16), String> {
    let svc = format!("{cluster_name}-postgresql");

    let mut child = Command::new(kubectl)
        .args([
            "port-forward",
            &format!("svc/{svc}"),
            &format!(":{REMOTE_PORT}"),
        ])
        .arg("--kubeconfig")
        .arg(kubeconfig)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("kubectl port-forward failed: {e}"))?;

    // kubectl prints "Forwarding from 127.0.0.1:XXXXX -> 5432" to stderr
    let stderr = child
        .stderr
        .take()
        .ok_or("port-forward stderr not captured")?;

    use std::io::Read;
    let mut buf = [0u8; 256];
    let mut port_str = String::new();
    let mut reader = std::io::BufReader::new(stderr);
    let mut total = 0;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            continue;
        }
        total += n;
        let s = String::from_utf8_lossy(&buf[..n]);
        port_str.push_str(&s);
        if let Some(port) = parse_forwarding_port(&port_str) {
            return Ok((child, port));
        }
        if total > 512 {
            break;
        }
    }

    let _ = child.kill();
    Err("could not determine local port from kubectl port-forward output".to_string())
}

fn parse_forwarding_port(output: &str) -> Option<u16> {
    // "Forwarding from 127.0.0.1:12345 -> 5432" or "[::1]:12345 -> 5432"
    let rest = output.find("127.0.0.1:")?;
    let after = &output[rest + "127.0.0.1:".len()..];
    let end = after.find(|c: char| !c.is_ascii_digit())?;
    after[..end].parse().ok()
}
