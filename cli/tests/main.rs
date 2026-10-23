//! Every integration test of `tsk`, in one test binary. Each file is a module, so the
//! harness runs all of the tests in one thread pool and the files share the template
//! ledgers of `common`, which are built once per run. Run one file with
//! `cargo test -p tsk-bin --test integration -- <module>::`, for example `-- ledger::`.
//! A test is named `<module>::<test>`.

mod common;

mod auto_update;
mod background;
mod broken_pipe;
mod conformance;
mod dolt_server;
mod events;
mod git_protocol;
mod hooks;
mod install_plugin;
mod ledger;
mod ledger_refs;
mod migrate_docs;
mod nexus;
mod nexus_cmd;
mod self_update;
mod thread;
mod transport;
