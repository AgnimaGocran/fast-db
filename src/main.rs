//! fdb — CLI for quick database cluster deployment via kbcli/kubectl.

mod cluster;
mod config;
mod credentials;
mod expose;
mod service;
mod tools;

use config::{load_config, load_kubeconfig};
use service::ServiceType;
use std::path::PathBuf;

fn main() {
    if let Err(e) = run() {
        eprintln!("fdb: {e}");
        std::process::exit(1);
    }
}

#[derive(Debug)]
enum CliCommand {
    Create {
        service: ServiceType,
        name: String,
        kubeconfig: Option<PathBuf>,
        replicas: Option<u32>,
        storage: Option<String>,
        cpu: Option<String>,
        memory: Option<String>,
    },
    Delete {
        name: String,
        kubeconfig: Option<PathBuf>,
        yes: bool,
    },
    List {
        kubeconfig: Option<PathBuf>,
    },
}

fn run() -> Result<(), String> {
    let cmd = parse_args()?;

    match cmd {
        CliCommand::Create {
            service,
            name,
            kubeconfig,
            replicas,
            storage,
            cpu,
            memory,
        } => run_create(service, &name, kubeconfig, replicas, storage, cpu, memory),
        CliCommand::Delete { name, kubeconfig, yes } => run_delete(&name, kubeconfig, yes),
        CliCommand::List { kubeconfig } => run_list(kubeconfig),
    }
}

fn parse_args() -> Result<CliCommand, String> {
    let mut kubeconfig: Option<PathBuf> = None;
    let mut replicas: Option<u32> = None;
    let mut storage: Option<String> = None;
    let mut cpu: Option<String> = None;
    let mut memory: Option<String> = None;
    let mut yes = false;
    let mut positional: Vec<String> = Vec::new();

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next().map_err(|e| e.to_string())? {
        match arg {
            lexopt::Arg::Long("kubeconfig") => {
                let val = parser.value().map_err(|e| e.to_string())?;
                kubeconfig = Some(PathBuf::from(val.to_string_lossy().into_owned()));
            }
            lexopt::Arg::Short('y') | lexopt::Arg::Long("yes") => yes = true,
            lexopt::Arg::Long("replicas") => {
                let val = parser.value().map_err(|e| e.to_string())?;
                let s = val.to_string_lossy();
                replicas = Some(s.parse().map_err(|_| format!("invalid --replicas: {s}"))?);
            }
            lexopt::Arg::Long("storage") => {
                let val = parser.value().map_err(|e| e.to_string())?;
                storage = Some(val.to_string_lossy().into_owned());
            }
            lexopt::Arg::Long("cpu") => {
                let val = parser.value().map_err(|e| e.to_string())?;
                cpu = Some(val.to_string_lossy().into_owned());
            }
            lexopt::Arg::Long("memory") => {
                let val = parser.value().map_err(|e| e.to_string())?;
                memory = Some(val.to_string_lossy().into_owned());
            }
            lexopt::Arg::Value(val) => {
                positional.push(val.to_string_lossy().into_owned());
            }
            _ => return Err(format!("unexpected argument: {arg:?}")),
        }
    }

    if positional.is_empty() {
        return Err(usage());
    }

    match positional[0].as_str() {
        "create" => {
            if positional.len() != 3 {
                return Err("usage: fdb create <postgresql|redis|rabbitmq|qdrant> <name> [--kubeconfig PATH] [--replicas N] [--storage SIZE] [--cpu CPU] [--memory MEM]".to_string());
            }
            let service = positional[1].parse::<ServiceType>()?;
            let name = positional[2].clone();
            Ok(CliCommand::Create {
                service,
                name,
                kubeconfig,
                replicas,
                storage,
                cpu,
                memory,
            })
        }
        "delete" => {
            if positional.len() != 2 {
                return Err("usage: fdb delete <name> [--kubeconfig PATH] [-y|--yes]".to_string());
            }
            let name = positional[1].clone();
            Ok(CliCommand::Delete {
                name,
                kubeconfig,
                yes,
            })
        }
        "list" => {
            if positional.len() != 1 {
                return Err("usage: fdb list [--kubeconfig PATH]".to_string());
            }
            Ok(CliCommand::List { kubeconfig })
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: fdb create <postgresql|redis|rabbitmq|qdrant> <name> [options]
       fdb delete <name> [-y|--yes] [--kubeconfig PATH]
       fdb list [--kubeconfig PATH]"
        .to_string()
}

fn run_create(
    service: ServiceType,
    cluster_name: &str,
    kubeconfig_override: Option<PathBuf>,
    replicas_override: Option<u32>,
    storage_override: Option<String>,
    cpu_override: Option<String>,
    memory_override: Option<String>,
) -> Result<(), String> {
    let config = load_config(
        service,
        kubeconfig_override,
        replicas_override,
        storage_override,
        cpu_override,
        memory_override,
    );

    tools::ensure_tools()?;
    let kubectl = tools::resolve_kubectl()?;
    let kbcli = tools::resolve_kbcli()?;

    let started = chrono::Local::now();
    let kubeconfig_display = config.kubeconfig.display().to_string();
    println!(
        "Creating {} cluster \"{cluster_name}\" (replicas={}, storage={} Gi, cpu={}, memory={} Gi)",
        service.kbcli_name(),
        config.replicas,
        config.storage.trim_end_matches("Gi").trim_end_matches("gi").trim(),
        config.cpu,
        config.memory.trim_end_matches("Gi").trim_end_matches("gi").trim()
    );
    println!("  kubeconfig: {kubeconfig_display}");
    println!("  started: {}", started.format("%Y-%m-%d %H:%M:%S"));
    println!();

    cluster::create_cluster(
        &kbcli,
        service,
        cluster_name,
        &config.kubeconfig,
        config.replicas,
        &config.storage,
        &config.cpu,
        &config.memory,
    )?;

    cluster::wait_until_running(&kbcli, cluster_name, &config.kubeconfig)?;

    let password = credentials::get_password(
        &kubectl,
        service,
        cluster_name,
        &config.kubeconfig,
    )?;

    let user = service.default_user();

    let (host, port) = match (
        expose::server_host_from_kubeconfig(&kubectl, &config.kubeconfig),
        expose::ensure_nodeport_and_get_port(&kubectl, service, cluster_name, &config.kubeconfig),
    ) {
        (Ok(h), Ok(p)) => (h, p),
        (Err(e), _) => {
            eprintln!("warning: could not get server host from kubeconfig: {e}");
            (String::new(), 0)
        }
        (_, Err(e)) => {
            eprintln!("warning: could not expose NodePort: {e}");
            (String::new(), 0)
        }
    };

    println!();
    println!("Cluster \"{cluster_name}\" is running.");
    println!();
    println!("Connection details:");
    if !host.is_empty() && port != 0 {
        let connection_string = service.connection_string(
            user,
            password.as_deref(),
            &host,
            port,
        );
        println!("  Host:              {host}");
        println!("  Port:              {port}");
        println!("  User:              {user}");
        if let Some(ref p) = password {
            println!("  Password:          {p}");
        }
        println!("  Connection string: {connection_string}");
    } else {
        println!("  User:     {user}");
        if let Some(ref p) = password {
            println!("  Password: {p}");
        }
        println!("  (Host/Port: enable NodePort or check kubeconfig)");
    }

    Ok(())
}

fn run_delete(name: &str, kubeconfig_override: Option<PathBuf>, yes: bool) -> Result<(), String> {
    let kubeconfig = load_kubeconfig(kubeconfig_override);
    tools::ensure_tools()?;
    let kubectl = tools::resolve_kubectl()?;
    let kbcli = tools::resolve_kbcli()?;
    cluster::delete_cluster(&kbcli, &kubectl, name, &kubeconfig, yes)?;
    println!("Cluster \"{name}\" deleted.");
    Ok(())
}

fn run_list(kubeconfig_override: Option<PathBuf>) -> Result<(), String> {
    let kubeconfig = load_kubeconfig(kubeconfig_override);
    tools::ensure_tools()?;
    let kbcli = tools::resolve_kbcli()?;
    cluster::list_clusters(&kbcli, &kubeconfig)?;
    Ok(())
}
