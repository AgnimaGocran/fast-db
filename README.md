# Fast DB

CLI to create and manage database clusters in Kubernetes via [KubeBlocks](https://kubeblocks.io/) (kbcli). Supports PostgreSQL, Redis, RabbitMQ, and Qdrant. Creates a NodePort service so you can connect from outside the cluster.

## Prerequisites

On the target Kubernetes cluster you must **install KubeBlocks** and **configure the addons** for the databases you plan to use (PostgreSQL, Redis, RabbitMQ, Qdrant). fdb only creates clusters via kbcli; it does not install KubeBlocks or addons. See the [KubeBlocks documentation](https://kubeblocks.io/docs) for installation and addon setup.

## Quick start

Install fdb from this repository:

```bash
cargo install --git https://github.com/AgnimaGocran/fast-db
```

Ensure `~/.cargo/bin` is in your `PATH`, then run for example:

```bash
fdb list
fdb create postgresql mydb
```

(Use your cluster's kubeconfig via `--kubeconfig` or `KUBECONFIG` if needed.)

## Usage

### Create a cluster

```bash
fdb create <postgresql|redis|rabbitmq|qdrant> <name> [--kubeconfig PATH] [--replicas N] [--storage SIZE] [--cpu CPU] [--memory MEM]
```

Examples:

```bash
fdb create postgresql mydb
fdb create redis mycache --replicas 1 --storage 1
fdb create rabbitmq myqueue --memory 1
fdb create qdrant myvector --storage 5
```

- **name** — cluster name (e.g. `mydb`).
- **--kubeconfig** — path to kubeconfig (overrides config file).
- **--replicas**, **--storage**, **--cpu**, **--memory** — override values from config.

### Delete a cluster

```bash
fdb delete <name> [--kubeconfig PATH] [-y|--yes]
```

- Without `-y`/`--yes`, fdb asks for confirmation.
- With `-y` or `--yes`, the cluster is deleted without prompting (same as kbcli `--auto-approve`).

### List clusters

```bash
fdb list [--kubeconfig PATH]
```

Shows all KubeBlocks clusters and their status (same as `kbcli cluster list`).

## Config (fdb.toml)

Config is read from (first match wins):

1. `./fdb.toml` in the current directory
2. `~/.fdb/fdb.toml`

Example with all optional sections:

```toml
[kubernetes]
kubeconfig = "~/.kube/config"

[postgresql]
replicas = 1
storage = 2
cpu = 0.5
memory = 0.8

[redis]
replicas = 1
storage = 1
cpu = 0.5
memory = 0.5

[rabbitmq]
replicas = 1
storage = 2
cpu = 0.5
memory = 1

[qdrant]
replicas = 1
storage = 5
cpu = 0.5
memory = 1
```

All fields are optional; defaults apply if omitted.

## Output

After a cluster is created, fdb prints connection details: host (from kubeconfig), NodePort, user, password (when applicable), and a connection string:

- **PostgreSQL**: `postgresql://user:pass@host:port/postgres`
- **Redis**: `redis://:pass@host:6379`
- **RabbitMQ**: `amqp://user:pass@host:5672/`
- **Qdrant**: `http://host:6333`

It creates a separate NodePort service (`<name>-<service>-external`) so the cluster is reachable from outside; ensure the NodePort is allowed by your firewall.

## Tools

fdb uses `kubectl` and `kbcli`. If they are not in your `PATH`, fdb **will download them automatically** and place them in `~/.fdb/bin` (or `$FDB_HOME/bin` if `FDB_HOME` is set). You do not need to install kubectl or kbcli yourself.

## Build

```bash
cargo build --release
```

## License

MIT