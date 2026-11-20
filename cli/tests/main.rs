//! Every integration test of `tsk`, in one test binary. Each file is a module, so the
//! harness runs all of the tests in one thread pool and the files share the template
//! ledgers of `common`, which are built once per run. Run one file with
//! `cargo test -p tsk-bin --test integration -- <module>::`, for example `-- ledger::`.
//! A test is named `<module>::<test>`.

mod common;

mod attribution;
mod auto_update;
mod background;
mod broken_pipe;
mod concurrent;
mod conformance;
mod dolt_server;
mod events;
mod features;
mod git_protocol;
mod harness;
mod hooks;
mod install_plugin;
mod ledger;
mod ledger_refs;
mod migrate_docs;
mod migrate_home;
mod nexus;
mod nexus_cmd;
mod release_notes;
mod self_update;
mod shell;
mod signing;
mod thread;
mod tmux;
mod transport;
