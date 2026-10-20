//! End-to-end local runtime tests; no sockets, database, or login service.
#[path = "runtime_cases/boundaries.rs"]
mod boundaries;
#[path = "runtime_cases/flow.rs"]
mod flow;
#[path = "runtime_cases/policy.rs"]
mod policy;
#[path = "runtime_cases/support.rs"]
mod support;
