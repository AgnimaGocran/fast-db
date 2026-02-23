//! Configuration from fdb.toml with defaults.

use crate::service::ServiceType;
use serde::Deserialize;
use std::path::PathBuf;

const DEFAULT_KUBECONFIG: &str = "~/.kube/config";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct KubernetesSection {
    kubeconfig: Option<String>,
}

/// Deserialize TOML value as string: "2Gi", 2, or 0.8 all become a string for storage/memory.
fn deser_string_or_number<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        S(String),
        I(i64),
        F(f64),
    }
    let v = Option::<StringOrNumber>::deserialize(deserializer)?;
    Ok(v.map(|x| match x {
        StringOrNumber::S(s) => s,
        StringOrNumber::I(i) => i.to_string(),
        StringOrNumber::F(f) => f.to_string(),
    }))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PostgresqlSection {
    replicas: Option<u32>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    storage: Option<String>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    cpu: Option<String>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    memory: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RedisSection {
    replicas: Option<u32>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    storage: Option<String>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    cpu: Option<String>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    memory: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RabbitmqSection {
    replicas: Option<u32>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    storage: Option<String>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    cpu: Option<String>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    memory: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct QdrantSection {
    replicas: Option<u32>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    storage: Option<String>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    cpu: Option<String>,
    #[serde(default, deserialize_with = "deser_string_or_number")]
    memory: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct FdbToml {
    kubernetes: Option<KubernetesSection>,
    postgresql: Option<PostgresqlSection>,
    redis: Option<RedisSection>,
    rabbitmq: Option<RabbitmqSection>,
    qdrant: Option<QdrantSection>,
}

/// Merged configuration (fdb.toml + CLI overrides).
#[derive(Debug, Clone)]
pub struct Config {
    pub kubeconfig: PathBuf,
    pub replicas: u32,
    pub storage: String,
    pub cpu: String,
    pub memory: String,
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(path.trim_start_matches("~/"));
        }
    }
    if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(path)
}

/// Default (replicas, storage, cpu, memory) per service type.
fn defaults_for_service(service: ServiceType) -> (u32, String, String, String) {
    match service {
        ServiceType::PostgreSQL => (1, "2Gi".to_string(), "0.5".to_string(), "0.8Gi".to_string()),
        ServiceType::Redis => (1, "1Gi".to_string(), "0.5".to_string(), "0.5Gi".to_string()),
        ServiceType::RabbitMQ => (1, "2Gi".to_string(), "0.5".to_string(), "1Gi".to_string()),
        ServiceType::Qdrant => (1, "5Gi".to_string(), "0.5".to_string(), "1Gi".to_string()),
    }
}

/// Load config from fdb.toml (current dir then ~/.fdb/fdb.toml), then apply CLI overrides.
pub fn load_config(
    service: ServiceType,
    kubeconfig_override: Option<PathBuf>,
    replicas_override: Option<u32>,
    storage_override: Option<String>,
    cpu_override: Option<String>,
    memory_override: Option<String>,
) -> Config {
    let mut kubeconfig = expand_tilde(DEFAULT_KUBECONFIG);
    let (mut replicas, mut storage, mut cpu, mut memory) = defaults_for_service(service);

    if let Some(toml_config) = load_fdb_toml() {
        if let Some(k8s) = toml_config.kubernetes {
            if let Some(k) = k8s.kubeconfig {
                kubeconfig = expand_tilde(&k);
            }
        }
        match service {
            ServiceType::PostgreSQL => {
                if let Some(pg) = toml_config.postgresql {
                    if let Some(r) = pg.replicas {
                        replicas = r;
                    }
                    if let Some(s) = pg.storage {
                        storage = s;
                    }
                    if let Some(c) = pg.cpu {
                        cpu = c;
                    }
                    if let Some(m) = pg.memory {
                        memory = m;
                    }
                }
            }
            ServiceType::Redis => {
                if let Some(r) = toml_config.redis {
                    if let Some(v) = r.replicas {
                        replicas = v;
                    }
                    if let Some(s) = r.storage {
                        storage = s;
                    }
                    if let Some(c) = r.cpu {
                        cpu = c;
                    }
                    if let Some(m) = r.memory {
                        memory = m;
                    }
                }
            }
            ServiceType::RabbitMQ => {
                if let Some(r) = toml_config.rabbitmq {
                    if let Some(v) = r.replicas {
                        replicas = v;
                    }
                    if let Some(s) = r.storage {
                        storage = s;
                    }
                    if let Some(c) = r.cpu {
                        cpu = c;
                    }
                    if let Some(m) = r.memory {
                        memory = m;
                    }
                }
            }
            ServiceType::Qdrant => {
                if let Some(q) = toml_config.qdrant {
                    if let Some(v) = q.replicas {
                        replicas = v;
                    }
                    if let Some(s) = q.storage {
                        storage = s;
                    }
                    if let Some(c) = q.cpu {
                        cpu = c;
                    }
                    if let Some(m) = q.memory {
                        memory = m;
                    }
                }
            }
        }
    }

    if let Some(k) = kubeconfig_override {
        kubeconfig = k;
    }
    if let Some(r) = replicas_override {
        replicas = r;
    }
    if let Some(s) = storage_override {
        storage = s;
    }
    if let Some(c) = cpu_override {
        cpu = c;
    }
    if let Some(m) = memory_override {
        memory = m;
    }

    Config {
        kubeconfig,
        replicas,
        storage,
        cpu,
        memory,
    }
}

/// Load only kubeconfig (for list/delete when no service section needed).
pub fn load_kubeconfig(kubeconfig_override: Option<PathBuf>) -> PathBuf {
    let mut kubeconfig = expand_tilde(DEFAULT_KUBECONFIG);
    if let Some(toml_config) = load_fdb_toml() {
        if let Some(k8s) = toml_config.kubernetes {
            if let Some(k) = k8s.kubeconfig {
                kubeconfig = expand_tilde(&k);
            }
        }
    }
    kubeconfig_override.unwrap_or(kubeconfig)
}

fn load_fdb_toml() -> Option<FdbToml> {
    if let Ok(dir) = std::env::current_dir() {
        let local = dir.join("fdb.toml");
        if local.is_file() {
            if let Ok(content) = std::fs::read_to_string(&local) {
                if let Ok(cfg) = toml::from_str(&content) {
                    return Some(cfg);
                }
            }
        }
    }
    let global = expand_tilde("~/.fdb/fdb.toml");
    if global.is_file() {
        std::fs::read_to_string(&global).ok().and_then(|c| toml::from_str(&c).ok())
    } else {
        None
    }
}
