//! The local move of the ledger state of tsk 0.69.0 and earlier to a ledger home (ADR 0035).
//!
//! Before, a clone kept `<state>/repos/<clone-id>/` with `ledger-db`, `docs` and
//! `ledger-remote`, the last two as linked worktrees of the code repo, and the code repo held
//! the ledger objects under `refs/tsk-fetch/*`. The move takes the store and the worktrees to
//! `<state>/ledgers/<key>/`, whose worktrees belong to a bare clone that tsk owns. It runs at
//! the first ledger command of the new tsk, with no network, and no step by hand.
//!
//! Rules of the move:
//!
//! - Nothing is deleted before its copy is verified. A legacy directory is renamed into
//!   `repos/<clone-id>/legacy-leftovers/`, never removed. A second copy of one repo's ledger
//!   (two clones on one machine) is merged in and kept under `<home>/preserved/<clone-id>/`.
//! - Every step probes its own state, so a run that stopped halfway is finished by the next
//!   one. A journal under `<home>/.migrating/<clone-id>/` marks the steps that copy into a
//!   directory a re-run would otherwise take for a second copy.
//! - Three locks, always in this order: `<state>/migrate.lock`, the push lock of the old store,
//!   the push lock of the home. A second process waits at the first and then finds nothing to
//!   move.

use std::path::{Path, PathBuf};

use super::clone_lock;
use super::dolt::Dolt;
use super::git::Git;
use super::home::{self, Home, HomeKey, Marker};
use super::location::Repo;
use super::refs::LedgerRefs;
use super::store::server;
use super::transport;

/// The state-wide lock of the move.
pub const MIGRATE_LOCK: &str = "migrate.lock";
/// Where the directories of the old layout go, in the clone state directory.
pub const LEFTOVERS_DIR: &str = "legacy-leftovers";
const JOURNAL_DIR: &str = ".migrating";
const PRESERVED_DIR: &str = "preserved";
/// Test seam: the process exits after the named step.
pub const STOP_AFTER_VAR: &str = "TSK_TEST_MIGRATE_STOP_AFTER";
/// Test seam: one copied file is cut short, to prove the verify gate.
pub const CORRUPT_VAR: &str = "TSK_TEST_MIGRATE_CORRUPT_COPY";

/// The steps of the move, in order.
pub const STEPS: [&str; 10] = [
    "servers",
    "git",
    "dolt",
    "docs",
    "ledger",
    "url",
    "scratch",
    "verify",
    "unregister",
    "finish",
];

const FETCHED_REFS: [&str; 2] = ["refs/tsk-fetch/ledger", "refs/tsk-fetch/docs"];
const MIGRATE_PIN_PREFIX: &str = "refs/tsk/migrate/";
const PRESERVED_PIN_PREFIX: &str = "refs/tsk/preserved/";

/// True when `clone_dir` (`<state>/repos/<clone-id>`) holds the ledger state of 0.69.0 or
/// earlier.
pub fn has_legacy_state(clone_dir: &Path) -> bool {
    ["ledger-db", "docs", "ledger-remote"]
        .iter()
        .any(|name| clone_dir.join(name).is_dir())
        // A move that stopped after its directories were taken away still has to finish.
        || clone_dir.join(KEY_FILE).is_file()
}

/// The file in the clone state directory that names the home a move is going to. It is written
/// before the first step, so a run that stopped halfway finds its way back, and removed by the
/// last step.
const KEY_FILE: &str = ".legacy-key";

/// The key of the home the legacy store belongs to, from the ref it was made for: a nexus
/// ledger ref names the entry id, `tsk/ledger` names the clone. `None` when the ref cannot be
/// read.
pub fn legacy_key(clone_dir: &Path, clone_id: &str) -> Option<HomeKey> {
    if let Ok(text) = std::fs::read_to_string(clone_dir.join(KEY_FILE)) {
        if let Some(key) = HomeKey::from_dir_name(text.trim()) {
            return Some(key);
        }
    }
    let name = recorded_ref(clone_dir)?;
    key_of_ref(&name, clone_id)
}

fn key_of_ref(name: &str, clone_id: &str) -> Option<HomeKey> {
    for mode in [LedgerRefs::Branches, LedgerRefs::Custom] {
        if let Some(rest) = name.strip_prefix(mode.nexus_prefix()) {
            let id = rest.strip_suffix("/ledger").unwrap_or(rest);
            if !id.is_empty() && !id.contains('/') {
                return Some(HomeKey::Nexus(id.to_string()));
            }
            return None;
        }
        if name == mode.ledger_ref() {
            return Some(HomeKey::Clone(clone_id.to_string()));
        }
    }
    None
}

/// The ledger ref of the legacy store: the file beside the database, else the `git_ref` of
/// the remote in Dolt's own state.
fn recorded_ref(clone_dir: &Path) -> Option<String> {
    if let Some(name) = transport::read_ref_file(clone_dir) {
        return Some(name);
    }
    let state = std::fs::read_to_string(
        clone_dir
            .join("ledger-db")
            .join(".dolt")
            .join("repo_state.json"),
    )
    .ok()?;
    let state: serde_json::Value = serde_json::from_str(&state).ok()?;
    state
        .get("remotes")?
        .get(super::dolt::REMOTE)?
        .get("params")?
        .get("git_ref")?
        .as_str()
        .map(str::to_string)
}

fn step_error(home: &Home, step: &str, cause: &str) -> String {
    format!(
        "error: moving the ledger state of this clone to {} stopped at {}: {}.\n       \
         Nothing was lost; run the command again.",
        home.root().display(),
        step,
        cause.trim_start_matches("error: ")
    )
}

fn stop_here(step: &str) {
    if std::env::var(STOP_AFTER_VAR).is_ok_and(|v| v == step) {
        std::process::exit(97);
    }
}

/// Move the legacy state of clone `clone_id` into `home`, or finish a move that stopped.
pub fn run(repo: &Repo, state_root: &Path, home: &Home, clone_id: &str) -> Result<(), String> {
    let legacy = home::clone_state_dir(state_root, clone_id);
    if !has_legacy_state(&legacy) {
        return Ok(());
    }
    std::fs::create_dir_all(home.root())
        .map_err(|e| format!("error: could not create {}: {}", home.root().display(), e))?;
    let _migrate = clone_lock::take_file(&state_root.join(MIGRATE_LOCK))?;
    // Another process finished the move while this one waited.
    if !has_legacy_state(&legacy) {
        return Ok(());
    }
    let _old = clone_lock::take_file(&legacy.join(clone_lock::LOCK_FILE))?;
    let _new = clone_lock::take(&home.db())?;
    home::write_atomic(
        &legacy.join(KEY_FILE),
        &format!("{}\n", home.key().dir_name()),
    )?;

    let mover = Mover {
        repo,
        home,
        clone_id,
        legacy: &legacy,
        journal: home.root().join(JOURNAL_DIR).join(clone_id),
        copied: std::cell::RefCell::new(Vec::new()),
        notes: std::cell::RefCell::new(Vec::new()),
    };
    std::fs::create_dir_all(&mover.journal)
        .map_err(|e| step_error(home, "servers", &e.to_string()))?;
    for step in STEPS {
        mover
            .step(step)
            .map_err(|cause| step_error(home, step, &cause))?;
        stop_here(step);
    }
    for note in mover.notes.borrow().iter() {
        eprintln!("{}", note);
    }
    Ok(())
}

struct Mover<'a> {
    repo: &'a Repo,
    home: &'a Home,
    clone_id: &'a str,
    legacy: &'a Path,
    journal: PathBuf,
    /// The worktrees copied whole, as (step, legacy, new): the verify step compares them.
    copied: std::cell::RefCell<Vec<(String, PathBuf, PathBuf)>>,
    notes: std::cell::RefCell<Vec<String>>,
}

impl Mover<'_> {
    fn step(&self, step: &str) -> Result<(), String> {
        match step {
            "servers" => self.stop_servers(),
            "git" => self.make_git(),
            "dolt" => self.move_dolt(),
            "docs" => self.move_worktree("docs", &self.legacy.join("docs"), &self.home.docs()),
            "ledger" => self.move_worktree(
                "ledger",
                &self.legacy.join("ledger-remote"),
                &self.home.ledger_dir(),
            ),
            "url" => self.rewrite_url(),
            "scratch" => self.move_scratch(),
            "verify" => self.verify(),
            "unregister" => self.unregister(),
            "finish" => self.finish(),
            _ => Ok(()),
        }
    }

    fn marker(&self, name: &str) -> PathBuf {
        self.journal.join(name)
    }

    fn mark(&self, name: &str) -> Result<(), String> {
        std::fs::write(self.marker(name), b"")
            .map_err(|e| format!("could not write {}: {}", self.marker(name).display(), e))
    }

    fn note(&self, text: String) {
        self.notes.borrow_mut().push(text);
    }

    fn in_repo(&self) -> bool {
        matches!(self.home.key(), HomeKey::Clone(_))
    }

    fn bare(&self) -> Git {
        self.home.git()
    }

    // --- Steps ---

    fn stop_servers(&self) -> Result<(), String> {
        for db in [self.legacy.join("ledger-db"), self.home.db()] {
            if db.join(".dolt").is_dir() {
                server::stop(&db)?;
            }
        }
        Ok(())
    }

    fn make_git(&self) -> Result<(), String> {
        self.home.ensure_git(self.repo, self.in_repo()).map(|_| ())
    }

    fn move_dolt(&self) -> Result<(), String> {
        let legacy_db = self.legacy.join("ledger-db");
        if !legacy_db.exists() {
            return Ok(());
        }
        let db = self.home.db();
        if !db.exists() {
            return std::fs::rename(&legacy_db, &db).map_err(|e| {
                format!(
                    "could not rename {} to {}: {} (the state directory must be on one filesystem)",
                    legacy_db.display(),
                    db.display(),
                    e
                )
            });
        }
        self.fold_dolt(&legacy_db)
    }

    /// A second copy of the ledger of one repo, from another clone on this machine: its
    /// commits and its working set are merged into the home's store, and the copy is kept.
    fn fold_dolt(&self, legacy_db: &Path) -> Result<(), String> {
        let db = Dolt::new(self.home.db());
        let old = Dolt::new(legacy_db);
        let xfer = self.journal.join("xfer");
        // Re-entrant: clear what an earlier, interrupted attempt left.
        let _ = db.output(["remote", "remove", "legacy"]);
        let _ = db.output(["merge", "--abort"]);
        let _ = std::fs::remove_dir_all(&xfer);
        let _ = old.output(["remote", "remove", "tsk-xfer"]);

        if ledger_id_of(&db) != ledger_id_of(&old) && ledger_id_of(&old).is_some() {
            return self.set_aside_dolt(
                legacy_db,
                "it holds a different ledger than the shared store",
            );
        }
        // Dolt refuses to merge into a dirty working set, and the commit lets a failed merge
        // abort without losing anything.
        commit_working_set(
            &old,
            &format!(
                "Preserve the working set of clone {} before the move",
                self.clone_id
            ),
        )?;
        commit_working_set(
            &db,
            &format!(
                "Preserve the working set before folding clone {}",
                self.clone_id
            ),
        )?;
        std::fs::create_dir_all(&xfer)
            .map_err(|e| format!("could not create {}: {}", xfer.display(), e))?;
        let url = transport::file_url(&xfer);
        old.run(["remote", "add", "tsk-xfer", url.as_str()])?;
        old.run(["push", "tsk-xfer", super::dolt::BRANCH])?;
        let _ = old.output(["remote", "remove", "tsk-xfer"]);
        db.run(["remote", "add", "legacy", url.as_str()])?;
        db.run(["fetch", "legacy"])?;
        let remote_ref = format!("legacy/{}", super::dolt::BRANCH);
        let mut renumbered = Vec::new();
        let merged =
            super::store::dolt::merge_renumbering_for_move(&db, &remote_ref, &mut renumbered);
        let _ = db.output(["remote", "remove", "legacy"]);
        let _ = std::fs::remove_dir_all(&xfer);
        match merged {
            Ok(()) => {
                self.keep_copy(legacy_db)?;
                self.note(format!(
                    "note: merged the ledger store of another clone of this repo into {}; the copy is kept in {} until you delete it",
                    self.home.db().display(),
                    self.home.root().join(PRESERVED_DIR).join(self.clone_id).display()
                ));
                Ok(())
            }
            Err(error) => {
                self.keep_copy(legacy_db)?;
                self.note(format!(
                    "note: the ledger store of another clone of this repo conflicts with {}; it is kept in {} and nothing was merged:\n      {}",
                    self.home.db().display(),
                    self.home.root().join(PRESERVED_DIR).join(self.clone_id).display(),
                    error.lines().next().unwrap_or("").trim_start_matches("error: ")
                ));
                Ok(())
            }
        }
    }

    fn set_aside_dolt(&self, legacy_db: &Path, why: &str) -> Result<(), String> {
        self.keep_copy(legacy_db)?;
        self.note(format!(
            "note: the ledger store of another clone is kept in {} and not merged: {}",
            self.home
                .root()
                .join(PRESERVED_DIR)
                .join(self.clone_id)
                .display(),
            why
        ));
        Ok(())
    }

    /// Rename a legacy store into `<home>/preserved/<clone-id>/`.
    fn keep_copy(&self, legacy_db: &Path) -> Result<(), String> {
        let target = self.home.root().join(PRESERVED_DIR).join(self.clone_id);
        std::fs::create_dir_all(&target)
            .map_err(|e| format!("could not create {}: {}", target.display(), e))?;
        let name = legacy_db
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "ledger-db".to_string());
        let mut to = target.join(&name);
        let mut n = 1;
        while to.exists() {
            to = target.join(format!("{}.{}", name, n));
            n += 1;
        }
        std::fs::rename(legacy_db, &to)
            .map_err(|e| format!("could not rename {}: {}", legacy_db.display(), e))
    }

    /// The documents worktree and the file remote worktree: first copy, second copy, or a
    /// worktree git cannot read.
    fn move_worktree(&self, step: &str, legacy_wt: &Path, new: &Path) -> Result<(), String> {
        if !legacy_wt.exists() {
            return Ok(());
        }
        let name = legacy_wt
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let started = format!("{}.started", step);
        let done = format!("{}.done", step);
        let folded = format!("{}.folded", step);
        let verified = format!("{}.verified", step);
        if self.marker(&done).exists() {
            // Copied by an earlier run that stopped before the gate: the gate still runs. A
            // copy the gate passed is not compared again: another clone may have folded its
            // documents into it since.
            if !self.marker(&verified).exists() {
                self.copied.borrow_mut().push((
                    step.to_string(),
                    legacy_wt.to_path_buf(),
                    new.to_path_buf(),
                ));
            }
            return Ok(());
        }
        if self.marker(&folded).exists() {
            return Ok(());
        }
        let second_copy = new.exists() && !self.marker(&started).exists();
        if second_copy && step == "docs" {
            self.require_first_copy_finished()?;
        }
        let Some(sha) = self.head_of(legacy_wt) else {
            // Nothing in it can be read by git: keep the directory untouched.
            self.note(format!(
                "note: {} is not a readable git worktree; it is kept in {} and the ledger is fetched afresh",
                legacy_wt.display(),
                self.legacy.join(LEFTOVERS_DIR).join(&name).display()
            ));
            self.set_leftover(legacy_wt)?;
            return Ok(());
        };
        let bare = self.bare();
        let pin = format!("{}{}-{}", MIGRATE_PIN_PREFIX, self.clone_id, name);
        self.import_objects(&bare, &pin, &sha)?;
        if second_copy {
            if step == "docs" {
                self.fold_docs(&bare, legacy_wt, new, &sha)?;
            }
            // The file remote worktree of a second copy is a cache of the same remote.
            return self.mark(&folded);
        }
        self.mark(&started)?;
        if new.exists() {
            // A copy that an earlier run did not finish.
            let _ = bare.output(["worktree", "remove", "--force", &new.to_string_lossy()]);
            let _ = std::fs::remove_dir_all(new);
        }
        let _ = bare.output(["worktree", "prune"]);
        bare.run([
            "worktree",
            "add",
            "--detach",
            "--quiet",
            &new.to_string_lossy(),
            &sha,
        ])?;
        clear_tree(new)?;
        copy_tree(legacy_wt, new)?;
        if std::env::var(CORRUPT_VAR).is_ok_and(|v| v == step) {
            corrupt_one_file(new);
        }
        self.copied.borrow_mut().push((
            step.to_string(),
            legacy_wt.to_path_buf(),
            new.to_path_buf(),
        ));
        self.mark(&done)
    }

    /// A second copy is folded into a shared documents worktree only once the first copy there
    /// is complete and has passed the gate. A clone whose move stopped earlier has a journal
    /// with `docs.started` and no `docs.verified`: its resume would rebuild the worktree from
    /// its own copy and drop what was folded in.
    fn require_first_copy_finished(&self) -> Result<(), String> {
        let Some(parent) = self.journal.parent() else {
            return Ok(());
        };
        let Some(repos) = self.legacy.parent() else {
            return Ok(());
        };
        let Ok(entries) = std::fs::read_dir(parent) else {
            return Ok(());
        };
        for entry in entries.flatten() {
            let other = entry.file_name().to_string_lossy().into_owned();
            if other == self.clone_id {
                continue;
            }
            let journal = entry.path();
            if journal.join("docs.started").exists()
                && !journal.join("docs.verified").exists()
                && has_legacy_state(&repos.join(&other))
            {
                return Err(format!(
                    "the move of clone {} into this ledger home has not finished. Run a ledger command in that clone first, then run this one again",
                    other
                ));
            }
        }
        Ok(())
    }

    /// The commit a legacy worktree is at: from git, else from the administrative directory the
    /// code repo keeps for it.
    fn head_of(&self, wt: &Path) -> Option<String> {
        let spec = "HEAD^{commit}";
        if let Ok(sha) = Git::new(wt).run_line(["rev-parse", "--verify", spec]) {
            if self.code_has_commit(&sha) {
                return Some(sha);
            }
        }
        let text = std::fs::read_to_string(wt.join(".git")).ok()?;
        let admin = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
        let head = std::fs::read_to_string(admin.join("HEAD")).ok()?;
        let sha = head.trim().to_string();
        (sha.len() >= 40
            && sha.chars().all(|c| c.is_ascii_hexdigit())
            && self.code_has_commit(&sha))
        .then_some(sha)
    }

    fn code_has_commit(&self, sha: &str) -> bool {
        let spec = format!("{}^{{commit}}", sha);
        self.repo
            .git
            .output(["cat-file", "-e", spec.as_str()])
            .is_ok_and(|(_, out)| out.status.success())
    }

    /// Bring the objects of `sha` from the code repo into the bare clone with no network:
    /// the code repo's object directory is an alternate for one `repack`, which copies only
    /// the objects reachable from the bare clone's refs. The pin ref keeps them reachable.
    fn import_objects(&self, bare: &Git, pin: &str, sha: &str) -> Result<(), String> {
        let info = self.home.git_dir().join("objects").join("info");
        std::fs::create_dir_all(&info)
            .map_err(|e| format!("could not create {}: {}", info.display(), e))?;
        let alternates = info.join("alternates");
        let objects = self.repo.common_dir.join("objects");
        std::fs::write(&alternates, format!("{}\n", objects.display()))
            .map_err(|e| format!("could not write {}: {}", alternates.display(), e))?;
        let imported = bare
            .run(["update-ref", pin, sha])
            .and_then(|_| bare.run(["repack", "-a", "-d", "-q"]));
        let _ = std::fs::remove_file(&alternates);
        imported?;
        let spec = format!("{}^{{commit}}", sha);
        bare.run(["cat-file", "-e", spec.as_str()])?;
        Ok(())
    }

    /// A second copy of the documents: its uncommitted files become a commit, which is merged
    /// into the shared worktree. A conflict leaves the commit under a ref.
    fn fold_docs(
        &self,
        bare: &Git,
        legacy_wt: &Path,
        shared: &Path,
        sha: &str,
    ) -> Result<(), String> {
        let temp = self
            .home
            .root()
            .join(format!("docs.migrating-{}", self.clone_id));
        if temp.exists() {
            let _ = bare.output(["worktree", "remove", "--force", &temp.to_string_lossy()]);
            let _ = std::fs::remove_dir_all(&temp);
        }
        let _ = bare.output(["worktree", "prune"]);
        bare.run([
            "worktree",
            "add",
            "--detach",
            "--quiet",
            &temp.to_string_lossy(),
            sha,
        ])?;
        let result = (|| -> Result<String, String> {
            clear_tree(&temp)?;
            copy_tree(legacy_wt, &temp)?;
            let wt = Git::new(&temp);
            super::push::commit_changes(
                &wt,
                &format!(
                    "Preserve the uncommitted documents of clone {}",
                    self.clone_id
                ),
            )?;
            wt.run_line(["rev-parse", "--verify", "HEAD^{commit}"])
        })();
        let _ = bare.output(["worktree", "remove", "--force", &temp.to_string_lossy()]);
        let _ = std::fs::remove_dir_all(&temp);
        let _ = bare.output(["worktree", "prune"]);
        let commit = result?;
        let keep = format!("{}{}", PRESERVED_PIN_PREFIX, self.clone_id);
        bare.run(["update-ref", keep.as_str(), commit.as_str()])?;
        let shared_git = Git::new(shared);
        let (git, mut args) = crate::attribution::ledger_git(&shared_git)?;
        args.extend(["merge", "--no-edit", "--quiet", commit.as_str()].map(String::from));
        match git.run(args) {
            Ok(_) => {
                self.note(format!(
                    "note: merged the documents of another clone into {}",
                    shared.display()
                ));
            }
            Err(_) => {
                let _ = shared_git.output(["merge", "--abort"]);
                self.note(format!(
                    "note: the documents of another clone conflict with {}. They are kept as the commit {} under {}; see them with git -C {} show {}",
                    shared.display(),
                    &commit[..commit.len().min(12)],
                    keep,
                    self.home.git_dir().display(),
                    keep
                ));
            }
        }
        Ok(())
    }

    fn set_leftover(&self, wt: &Path) -> Result<(), String> {
        let name = wt
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dir = self.legacy.join(LEFTOVERS_DIR);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("could not create {}: {}", dir.display(), e))?;
        let mut to = dir.join(&name);
        let mut n = 1;
        while to.exists() {
            to = dir.join(format!("{}.{}", name, n));
            n += 1;
        }
        std::fs::rename(wt, &to).map_err(|e| format!("could not rename {}: {}", wt.display(), e))
    }

    /// Point the store's `origin` at the directory in the home. After the worktree step, so a
    /// directory that exists is canonical on both sides.
    fn rewrite_url(&self) -> Result<(), String> {
        rewrite_store_url(self.home)
    }

    fn move_scratch(&self) -> Result<(), String> {
        let scratch = self.legacy.join("scratch");
        if scratch.is_dir() {
            let target = self.home.scratch();
            if !target.exists() {
                std::fs::rename(&scratch, &target)
                    .map_err(|e| format!("could not move {}: {}", scratch.display(), e))?;
            } else {
                let into = target.join(format!("from-{}", self.clone_id));
                std::fs::create_dir_all(&target)
                    .map_err(|e| format!("could not create {}: {}", target.display(), e))?;
                std::fs::rename(&scratch, &into)
                    .map_err(|e| format!("could not move {}: {}", scratch.display(), e))?;
            }
        }
        let log = self.legacy.join("background-push.log");
        let log_to = self.home.root().join("background-push.log");
        if log.is_file() && !log_to.exists() {
            let _ = std::fs::rename(&log, &log_to);
        } else if log.is_file() {
            let _ = std::fs::remove_file(&log);
        }
        let ref_file = self.legacy.join(transport::REF_FILE);
        let ref_to = self.home.root().join(transport::REF_FILE);
        if ref_file.is_file() {
            if !ref_to.exists() && self.home.db().exists() {
                let _ = std::fs::rename(&ref_file, &ref_to);
            } else {
                let _ = std::fs::remove_file(&ref_file);
            }
        }
        // Caches that the new binary rebuilds.
        for derived in [
            "remote-tips",
            "ledger-refs",
            "background-push.result",
            "dolt-server.pid",
            "dolt-server.info",
            "dolt-server.log",
            "dolt-server.lock",
            "dolt-server.config.yaml",
        ] {
            let _ = std::fs::remove_file(self.legacy.join(derived));
        }
        Ok(())
    }

    /// The gate: every file of a worktree that was copied matches its source, byte for byte,
    /// before anything is unregistered.
    fn verify(&self) -> Result<(), String> {
        for (_, from, to) in self.copied.borrow().iter() {
            if let Some(difference) = tree_difference(from, to)? {
                // The copy is not to be trusted: the next run makes it again from the source,
                // which this step has not touched.
                for name in ["docs.done", "ledger.done"] {
                    let _ = std::fs::remove_file(self.marker(name));
                }
                return Err(format!(
                    "verify: {} differs between {} and {}",
                    difference,
                    from.display(),
                    to.display()
                ));
            }
        }
        let steps: Vec<String> = self
            .copied
            .borrow()
            .iter()
            .map(|(step, _, _)| step.clone())
            .collect();
        for step in steps {
            self.mark(&format!("{}.verified", step))?;
        }
        Ok(())
    }

    /// Take the worktrees out of the code repo: rename each legacy directory into the
    /// leftovers, then remove only the administrative directories that pointed into the old
    /// state directory, then the fetch refs. A plain `git worktree prune` would also drop a
    /// worktree of the user's own that is missing.
    fn unregister(&self) -> Result<(), String> {
        // Decide which administrative directories are ours before the legacy directories are
        // renamed: git writes a `gitdir` file as a real path, and once the directory is gone
        // a link through a symbolic link cannot be resolved.
        let admin_root = self.repo.common_dir.join("worktrees");
        let mut ours: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&admin_root) {
            for entry in entries.flatten() {
                let admin = entry.path();
                let Ok(pointer) = std::fs::read_to_string(admin.join("gitdir")) else {
                    continue;
                };
                if points_under(pointer.trim(), self.legacy) {
                    ours.push(admin);
                }
            }
        }
        let mut leftovers = Vec::new();
        for name in ["docs", "ledger-remote"] {
            let dir = self.legacy.join(name);
            if dir.exists() {
                self.set_leftover(&dir)?;
                leftovers.push(name);
            }
        }
        for admin in ours {
            let _ = std::fs::remove_dir_all(&admin);
        }
        for name in FETCHED_REFS {
            let _ = self.repo.git.output(["update-ref", "-d", name]);
        }
        let bare = self.bare();
        if let Ok(listing) = bare.run(["for-each-ref", "--format=%(refname)", MIGRATE_PIN_PREFIX]) {
            for pin in listing.lines().filter(|l| !l.is_empty()) {
                let _ = bare.output(["update-ref", "-d", pin]);
            }
        }
        if !leftovers.is_empty() {
            let dir = self.legacy.join(LEFTOVERS_DIR);
            self.note(format!(
                "note: moved the ledger state of this clone to {}. The old directories are kept in {} ({} KB); delete them once the ledger is confirmed",
                self.home.root().display(),
                dir.display(),
                dir_size(&dir) / 1024
            ));
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), String> {
        let mut marker = Marker::read(self.home).unwrap_or_default();
        if !marker.migrated.iter().any(|c| c == self.clone_id) {
            marker.migrated.push(self.clone_id.to_string());
        }
        marker.write(self.home)?;
        let _ = std::fs::remove_dir_all(&self.journal);
        if let Some(parent) = self.journal.parent() {
            let _ = std::fs::remove_dir(parent);
        }
        home::write_home_file(self.repo, self.home)?;
        let _ = std::fs::remove_file(self.legacy.join(KEY_FILE));
        Ok(())
    }
}

/// Point the `origin` remote of a file-remote store at the `ledger` directory of its home.
/// The remote-tracking branch stays, so commits that were not pushed stay recognised. A store
/// on a git remote keeps its URL.
pub(crate) fn rewrite_store_url(home: &Home) -> Result<(), String> {
    let db = home.db();
    let state = db.join(".dolt").join("repo_state.json");
    let Some(old) = transport::store_url(&db) else {
        return Ok(());
    };
    if transport::Transport::of_url(&old) != transport::Transport::FileRemote {
        return Ok(());
    }
    let wanted = transport::file_url(&home.ledger_dir());
    if old == wanted {
        return Ok(());
    }
    let text = std::fs::read_to_string(&state)
        .map_err(|e| format!("could not read {}: {}", state.display(), e))?;
    let mut json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("could not parse {}: {}", state.display(), e))?;
    let Some(url) = json
        .get_mut("remotes")
        .and_then(|r| r.get_mut(super::dolt::REMOTE))
        .and_then(|r| r.get_mut("url"))
    else {
        return Ok(());
    };
    *url = serde_json::Value::String(wanted);
    let out = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    home::write_atomic(&state, &out)
}

fn ledger_id_of(db: &Dolt) -> Option<String> {
    db.rows("select v from manifest where k = 'ledger_id'")
        .ok()?
        .first()?
        .get("v")?
        .as_str()
        .map(str::to_string)
}

/// Commit the working set of a store, when it holds changes.
fn commit_working_set(db: &Dolt, message: &str) -> Result<(), String> {
    if db.count("select count(*) as c from dolt_status")? == 0 {
        return Ok(());
    }
    db.run(["add", "-A"])?;
    db.run(["commit", "-m", message]).map(|_| ())
}

/// True when the worktree link `pointer` (the content of an admin directory's `gitdir`
/// file, the path of the worktree's `.git` file) names a path under `dir`.
fn points_under(pointer: &str, dir: &Path) -> bool {
    let pointer = Path::new(pointer);
    if pointer.starts_with(dir) {
        return true;
    }
    match (std::fs::canonicalize(dir), pointer.parent()) {
        (Ok(real), Some(parent)) => {
            std::fs::canonicalize(parent).is_ok_and(|p| p.starts_with(real))
        }
        _ => false,
    }
}

// --- Trees ----------------------------------------------------------------------------

/// Remove everything in `dir` except `.git`.
fn clear_tree(dir: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(dir)
        .map_err(|e| format!("could not read {}: {}", dir.display(), e))?
        .flatten()
    {
        if entry.file_name() == ".git" {
            continue;
        }
        let path = entry.path();
        let kind = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("could not read {}: {}", path.display(), e))?
            .file_type();
        let removed = if kind.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.map_err(|e| format!("could not remove {}: {}", path.display(), e))?;
    }
    Ok(())
}

/// Copy every file of `from` except `.git` into `to`, keeping modes and symlinks.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(from)
        .map_err(|e| format!("could not read {}: {}", from.display(), e))?
        .flatten()
    {
        if entry.file_name() == ".git" && from.join(".git") == entry.path() {
            continue;
        }
        copy_entry(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(())
}

fn copy_entry(from: &Path, to: &Path) -> Result<(), String> {
    let kind = std::fs::symlink_metadata(from)
        .map_err(|e| format!("could not read {}: {}", from.display(), e))?
        .file_type();
    let fail = |e: std::io::Error| format!("could not copy {}: {}", from.display(), e);
    if kind.is_symlink() {
        let target = std::fs::read_link(from).map_err(fail)?;
        let _ = std::fs::remove_file(to);
        std::os::unix::fs::symlink(target, to).map_err(fail)
    } else if kind.is_dir() {
        std::fs::create_dir_all(to).map_err(fail)?;
        for entry in std::fs::read_dir(from).map_err(fail)?.flatten() {
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ()).map_err(fail)
    }
}

/// The first path (relative, `.git` at the top excluded) whose kind, executable bit or bytes
/// differ between two trees.
fn tree_difference(a: &Path, b: &Path) -> Result<Option<String>, String> {
    fn names(dir: &Path, top: bool) -> Result<Vec<std::ffi::OsString>, String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("could not read {}: {}", dir.display(), e))?
            .flatten()
            .map(|e| e.file_name())
            .filter(|n| !(top && n == ".git"))
            .collect();
        names.sort();
        Ok(names)
    }
    fn walk(a: &Path, b: &Path, rel: &Path, top: bool) -> Result<Option<String>, String> {
        let (left, right) = (names(a, top)?, names(b, top)?);
        if left != right {
            let odd = left
                .iter()
                .find(|n| !right.contains(n))
                .or_else(|| right.iter().find(|n| !left.contains(n)));
            let shown = match odd {
                Some(name) => rel.join(name),
                None => rel.to_path_buf(),
            };
            return Ok(Some(shown.display().to_string()));
        }
        for name in left {
            let (pa, pb) = (a.join(&name), b.join(&name));
            let shown = rel.join(&name);
            let (ma, mb) = (
                std::fs::symlink_metadata(&pa).map_err(|e| e.to_string())?,
                std::fs::symlink_metadata(&pb).map_err(|e| e.to_string())?,
            );
            let (ta, tb) = (ma.file_type(), mb.file_type());
            if ta.is_dir() != tb.is_dir() || ta.is_symlink() != tb.is_symlink() {
                return Ok(Some(shown.display().to_string()));
            }
            if ta.is_dir() {
                if let Some(found) = walk(&pa, &pb, &shown, false)? {
                    return Ok(Some(found));
                }
            } else if ta.is_symlink() {
                if std::fs::read_link(&pa).ok() != std::fs::read_link(&pb).ok() {
                    return Ok(Some(shown.display().to_string()));
                }
            } else {
                use std::os::unix::fs::PermissionsExt;
                let exec = |m: &std::fs::Metadata| m.permissions().mode() & 0o111 != 0;
                if exec(&ma) != exec(&mb)
                    || ma.len() != mb.len()
                    || std::fs::read(&pa).map_err(|e| e.to_string())?
                        != std::fs::read(&pb).map_err(|e| e.to_string())?
                {
                    return Ok(Some(shown.display().to_string()));
                }
            }
        }
        Ok(None)
    }
    walk(a, b, Path::new(""), true)
}

fn corrupt_one_file(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name() == ".git" {
                continue;
            }
            if path.is_file() && std::fs::metadata(&path).is_ok_and(|m| m.len() > 0) {
                let _ = std::fs::write(&path, b"");
                return;
            }
            if path.is_dir() {
                corrupt_one_file(&path);
            }
        }
    }
}

fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            match entry.metadata() {
                Ok(m) if m.is_dir() => total += dir_size(&entry.path()),
                Ok(m) => total += m.len(),
                Err(_) => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nexus_ref_names_the_entry_and_an_in_repo_ref_names_the_clone() {
        let key = |name: &str| key_of_ref(name, "tsk-1234abcd");
        assert_eq!(
            key("refs/heads/ledgers/work-api"),
            Some(HomeKey::Nexus("work-api".to_string()))
        );
        assert_eq!(
            key("refs/tsk/ledgers/work-api"),
            Some(HomeKey::Nexus("work-api".to_string()))
        );
        assert_eq!(
            key("refs/tsk/ledgers/work-api/ledger"),
            Some(HomeKey::Nexus("work-api".to_string()))
        );
        assert_eq!(
            key("refs/heads/tsk/ledger"),
            Some(HomeKey::Clone("tsk-1234abcd".to_string()))
        );
        assert_eq!(
            key("refs/tsk/ledger"),
            Some(HomeKey::Clone("tsk-1234abcd".to_string()))
        );
        assert_eq!(key("refs/heads/other"), None);
    }

    #[test]
    fn legacy_state_is_a_store_or_a_worktree_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_legacy_state(dir.path()));
        std::fs::write(dir.path().join("harness-names"), "a\n").unwrap();
        std::fs::create_dir(dir.path().join("hooks")).unwrap();
        assert!(!has_legacy_state(dir.path()));
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        assert!(has_legacy_state(dir.path()));
    }

    #[test]
    fn two_trees_that_differ_in_one_byte_are_told_apart() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        for tree in [&a, &b] {
            std::fs::create_dir_all(tree.join("sub")).unwrap();
            std::fs::write(tree.join("sub").join("x.md"), "same\n").unwrap();
            std::fs::write(tree.join(".git"), "gitdir: elsewhere\n").unwrap();
        }
        std::fs::write(b.join(".git"), "gitdir: other\n").unwrap();
        assert_eq!(tree_difference(&a, &b).unwrap(), None);
        std::fs::write(b.join("sub").join("x.md"), "diff\n").unwrap();
        assert_eq!(
            tree_difference(&a, &b).unwrap(),
            Some("sub/x.md".to_string())
        );
        std::fs::write(b.join("sub").join("x.md"), "same\n").unwrap();
        std::fs::write(b.join("extra"), "").unwrap();
        assert_eq!(tree_difference(&a, &b).unwrap(), Some("extra".to_string()));
    }

    #[test]
    fn a_copy_keeps_modes_and_leaves_the_git_file_behind() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        std::fs::create_dir_all(a.join("d")).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join(".git"), "gitdir: x\n").unwrap();
        std::fs::write(a.join("d").join("run.sh"), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(
            a.join("d").join("run.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        copy_tree(&a, &b).unwrap();
        assert!(!b.join(".git").exists());
        let mode = std::fs::metadata(b.join("d").join("run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
        assert_eq!(tree_difference(&a, &b).unwrap(), None);
    }

    #[test]
    fn a_link_into_the_old_state_directory_is_ours_and_one_elsewhere_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("repos").join("c1");
        std::fs::create_dir_all(state.join("docs")).unwrap();
        let ours = format!("{}/docs/.git", state.display());
        assert!(points_under(&ours, &state));
        assert!(!points_under("/somewhere/else/.git", &state));
    }
}
