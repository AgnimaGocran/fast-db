//! Service type (postgresql, redis, rabbitmq, qdrant) for kbcli and connection details.

use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceType {
    PostgreSQL,
    Redis,
    RabbitMQ,
    Qdrant,
}

impl ServiceType {
    /// Name used in kbcli: cluster create <name>.
    pub fn kbcli_name(&self) -> &'static str {
        match self {
            ServiceType::PostgreSQL => "postgresql",
            ServiceType::Redis => "redis",
            ServiceType::RabbitMQ => "rabbitmq",
            ServiceType::Qdrant => "qdrant",
        }
    }

    /// Default port for the service.
    pub fn default_port(&self) -> u16 {
        match self {
            ServiceType::PostgreSQL => 5432,
            ServiceType::Redis => 6379,
            ServiceType::RabbitMQ => 5672,
            ServiceType::Qdrant => 6333,
        }
    }

    /// Kubernetes secret name for account password (e.g. <cluster_name>-postgresql-account-postgres).
    pub fn secret_name(&self, cluster_name: &str) -> String {
        match self {
            ServiceType::PostgreSQL => format!("{cluster_name}-postgresql-account-postgres"),
            ServiceType::Redis => format!("{cluster_name}-redis-account-default"),
            ServiceType::RabbitMQ => format!("{cluster_name}-rabbitmq-account-root"),
            ServiceType::Qdrant => format!("{cluster_name}-qdrant-account-root"),
        }
    }

    /// Default user for connection string.
    pub fn default_user(&self) -> &'static str {
        match self {
            ServiceType::PostgreSQL => "postgres",
            ServiceType::Redis => "default",
            ServiceType::RabbitMQ => "root",
            ServiceType::Qdrant => "root",
        }
    }

    /// Whether this service typically has a password in K8s secret.
    pub fn has_password(&self) -> bool {
        match self {
            ServiceType::PostgreSQL | ServiceType::Redis | ServiceType::RabbitMQ => true,
            ServiceType::Qdrant => false,
        }
    }

    /// Build connection string for display.
    pub fn connection_string(
        &self,
        user: &str,
        password: Option<&str>,
        host: &str,
        port: u16,
    ) -> String {
        match self {
            ServiceType::PostgreSQL => {
                let pass = password.unwrap_or("");
                format!("postgresql://{user}:{pass}@{host}:{port}/postgres")
            }
            ServiceType::Redis => {
                let pass = password.unwrap_or("");
                if pass.is_empty() {
                    format!("redis://{host}:{port}")
                } else {
                    format!("redis://:{pass}@{host}:{port}")
                }
            }
            ServiceType::RabbitMQ => {
                let pass = password.unwrap_or("");
                format!("amqp://{user}:{pass}@{host}:{port}/")
            }
            ServiceType::Qdrant => format!("http://{host}:{port}"),
        }
    }

    /// Display name for port in Service YAML.
    pub fn port_name(&self) -> &'static str {
        match self {
            ServiceType::PostgreSQL => "postgresql",
            ServiceType::Redis => "redis",
            ServiceType::RabbitMQ => "rabbitmq",
            ServiceType::Qdrant => "qdrant",
        }
    }
}

impl FromStr for ServiceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &*s.to_lowercase() {
            "postgresql" | "postgres" | "pg" => Ok(ServiceType::PostgreSQL),
            "redis" => Ok(ServiceType::Redis),
            "rabbitmq" | "rabbit" => Ok(ServiceType::RabbitMQ),
            "qdrant" => Ok(ServiceType::Qdrant),
            _ => Err(format!(
                "unknown service type: {s} (supported: postgresql, redis, rabbitmq, qdrant)"
            )),
        }
    }
}
