# Deployment

This scaffold is not deployable as a game service. All applications exit after reporting metadata or explicitly rejecting unimplemented operations. No ports, containers, credentials, or orchestration definitions are provided.

Future deployment files must implement the private-database, single-world-writer, secret-handling, and recovery contracts in [ARCHITECTURE.md](../ARCHITECTURE.md) and [SECURITY.md](../SECURITY.md).
