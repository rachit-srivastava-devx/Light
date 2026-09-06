# TARGET — read before writing any install, CI, or packaging code

**fleet is a LOCAL CLI installed on a developer's own macOS machine. It is NOT a server.**

| yes | no |
|---|---|
| macOS only — Apple Silicon (`aarch64-apple-darwin`) primary, Intel (`x86_64-apple-darwin`) secondary | Linux, Windows, WSL, containers |
| single user, single machine, the user's own account | multi-tenant, multi-user, RBAC |
| runs when the user runs it | daemons, systemd, launchd services, uptime SLOs |
| local files under `$FLEET_STATE` (default `~/.local/state/fleet`) | databases, object storage, cloud buckets |
| keyless — the user's own `claude`/`codex` CLI credentials | API keys, service accounts, secrets managers |
| `fleet doctor` for "why is my machine unhappy" | health endpoints, readiness probes, metrics scrapers |
| `~/.local/bin`, Homebrew conventions | apt/yum, Docker images, Helm, k8s |

**Do not add server-shaped machinery.** No horizontal scaling, no load balancing, no replication,
no orchestration, no HA. If a design instinct comes from server ops, it is wrong here.
Observability means a local file the user can read plus a `doctor` command — not a collector.
