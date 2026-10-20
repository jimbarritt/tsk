//! `tsk ledger migrate-transport`: convert a ledger whose ref holds Dolt's own git
//! blobstore (transport `git-remote`, before 0.21.0) to a Dolt file remote carried by plain
//! git (transport `file-remote`, ADR 0017).
//!
//! The ledger store is the source: its history is pushed into the directory of the file
//! remote, which is a linked worktree reset to the ledger ref's current tip. The old tree
//! is removed from that worktree and the new files are added, and the result is one new
//! commit on top of the old tip. The push is plain: the old content is replaced by a normal
//! commit, never by a force. Every other clone moves to the file remote on its next fetch
//! (see `DoltStore::reroot`) and keeps its own commits, because Dolt commits have the same
//! hashes on both sides.

use std::path::Path;

use crate::ledger::dolt::{self, Dolt, BRANCH, REMOTE};
use crate::ledger::fetch::fetch_tip;
use crate::ledger::git::Git;
use crate::ledger::location::{dolt_db_path, state_root_from_env, Repo};
use crate::ledger::report_refresh;
use crate::ledger::store::dolt::{remote_tip, DoltStore};
use crate::ledger::tips::Tips;
use crate::ledger::transport::{self, Published, Transport};

const COMMIT_MESSAGE: &str = "Move the ledger to a Dolt file remote (transport file-remote)";
/// The temporary Dolt remote the history is pushed through.
const TEMP_REMOTE: &str = "migrate-transport";

pub fn run(cwd: &Path) -> Result<(), String> {
    Dolt::new(".").run(["version"])?;
    dolt::ensure_identity()?;

    let repo = Repo::discover(cwd)?;
    let state_root = state_root_from_env()?;
    let location = repo.location(&state_root)?;
    let store = DoltStore::open(cwd)?;

    if remote_tip(&repo.git, &location)?.is_none() {
        return Err(format!(
            "error: {} on {} holds no ledger, so there is nothing to migrate. A new ledger already uses a file remote",
            location.ref_name(),
            location.label()
        ));
    }

    // Bring the store up to origin. A store whose origin already holds a file remote moves
    // to it here.
    let (outcome, version) = store.refresh_data(false)?;
    let outcome = report_refresh(outcome);
    let db = Dolt::new(dolt_db_path(&state_root, &repo.ensure_clone_id()?));
    let dir = transport::remote_dir_beside(db.dir());
    let on_file_remote = transport::store_url(db.dir())
        .is_some_and(|url| Transport::of_url(&url) == Transport::FileRemote);
    if on_file_remote {
        eprintln!("note: this ledger already uses a file remote; nothing to migrate");
        println!("migrated-transport:already");
        return Ok(());
    }
    if version == dolt::LEGACY_LAYOUT_VERSION {
        return Err("error: this ledger stores its documents in Dolt (version 1). Run tsk ledger migrate-docs first".to_string());
    }
    if !outcome.pending.is_empty() {
        return Err(format!(
            "error: {} holds {} commits that are not on {}'s {}.\n       \
             Run tsk ledger push first, then tsk ledger migrate-transport",
            outcome.path.display(),
            outcome.pending.len(),
            location.label(),
            location.ref_name()
        ));
    }

    let old_tip = fetch_tip(&repo.git, &location)?.ok_or_else(|| {
        format!(
            "error: {} on {} has no commit",
            location.ref_name(),
            location.label()
        )
    })?;

    // The directory starts from the old tip, and the old tree leaves it.
    transport::sync_checkout(&repo, &dir, &old_tip)?;
    let wt = Git::new(&dir);
    wt.run(["rm", "-r", "-q", "--ignore-unmatch", "--", "."])?;
    wt.run(["clean", "-fdxq"])?;

    // The history of the store goes into the directory through a temporary remote.
    let _ = db.output(["remote", "remove", TEMP_REMOTE])?;
    db.run([
        "remote",
        "add",
        TEMP_REMOTE,
        transport::file_url(&dir).as_str(),
    ])?;
    let pushed = db.run(["push", TEMP_REMOTE, BRANCH]);
    let _ = db.output(["remote", "remove", TEMP_REMOTE])?;
    pushed?;
    std::fs::write(
        dir.join(transport::MARKER_FILE),
        format!("{}\n", Transport::FileRemote.as_str()),
    )
    .and_then(|()| {
        std::fs::write(
            dir.join(transport::GITIGNORE_FILE),
            transport::GITIGNORE_TEXT,
        )
    })
    .map_err(|e| format!("error: could not write in {}: {}", dir.display(), e))?;

    let commit = match transport::publish(&location, &dir, COMMIT_MESSAGE, Some(&old_tip))? {
        Published::Pushed(commit) | Published::Unchanged(commit) => commit,
        Published::Rejected(summary) => {
            return Err(format!(
                "error: {}'s {} moved while the ledger was converted: {}.\n       \
                 Nothing changed on origin. Run tsk ledger fetch, then tsk ledger migrate-transport again",
                location.label(),
                location.ref_name(),
                summary
            ));
        }
    };

    // The store follows: its origin is the directory now, and it keeps its commits.
    transport::point_store_at_file_remote(&db, &dir, location.ref_name())?;
    db.run(["fetch", REMOTE])?;
    let remote_head = db.hash_of(&format!("{}/{}", REMOTE, BRANCH))?;
    if remote_head != db.head_hash()? {
        return Err(format!(
            "error: the file remote holds {} but the ledger store is at {}. The ref now holds the file remote ({}); run tsk ledger status",
            remote_head,
            db.head_hash()?,
            commit
        ));
    }
    if let Some(clone_dir) = db.dir().parent() {
        Tips::new(clone_dir).set(location.remote(), location.ref_name(), Some(&commit));
    }
    eprintln!(
        "note: {}'s {} now holds a Dolt file remote; other clones move to it on their next fetch",
        location.label(),
        location.ref_name()
    );
    println!("migrated-transport:file-remote");
    println!("{}", commit);
    Ok(())
}
