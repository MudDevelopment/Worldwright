# Deployment

This repository includes a locally verified loopback Transport v0.1 server, but these deployment files do not define a production game service. No containers, orchestration, TLS termination, proxy trust, durable credentials, database, or recovery deployment is provided. Studio, Console, and persistence remain stubs.

Future deployment files must implement the private-database, single-world-writer, secret-handling, and recovery contracts in [ARCHITECTURE.md](../ARCHITECTURE.md) and [SECURITY.md](../SECURITY.md).
