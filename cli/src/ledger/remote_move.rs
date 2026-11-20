//! The move of a nexus ledger's refs to the new names (ADR 0035).
//!
//! Before, a nexus held `ledgers/<id>` and `ledgers/<id>-docs`. Now it holds
//! `ledgers/<id>/ledger` and `ledgers/<id>/docs`. Git cannot hold `ledgers/<id>` and
//! `ledgers/<id>/ledger` together, so the old ref goes first, or in the same push.
//!
//! The move is best-effort and per pair (the data ref and the documents ref move
//! independently), and it never loses a commit:
//!
//! 1. A backup ref `ledgers-backup/<id>/<ref>` is pushed first. At every point of the move one
//!    ref on the remote holds the tip.
//! 2. One push deletes the old ref and creates the new one, each under a lease on the old tip.
//!    `--atomic` is not used: git refuses it for a ref and its folder. A server that refuses the
//!    combined push gets the delete and the create one after the other.
//! 3. The backup goes when the new ref holds its tip.
//!
//! A refused delete, an unreachable remote or a lost lease leaves the ledger usable on the old
//! names for that process. Another machine with direct access finishes the move. The state of
//! each pair is read from one `git ls-remote`, so a machine that meets a half-finished move
//! resumes it: the old ref gone and the backup present never starts a new ledger.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::clone_lock;
use super::git::Git;
use super::home::{self, Home};
use super::location::{backup_prefix, LedgerLocation, RefLayout, Repo};
use super::push::parse_push_status;
use super::refs::LedgerRefs;

/// The file in the ledger home that records a move that did not finish: its error and time.
pub const LAST_FILE: &str = "remote-move.last";
/// A failed move is tried again once an hour, and at every `tsk ledger push`.
const RETRY_SECS: u64 = 3600;
const PIN_LEDGER: &str = "refs/tsk/migrate/ledger";
const PIN_DOCS: &str = "refs/tsk/migrate/docs";

static RETRY_NOW: AtomicBool = AtomicBool::new(false);
static FULL_CHECK: AtomicBool = AtomicBool::new(false);

/// `tsk ledger fetch` reads the remote's refs of this ledger even when the entry is marked, so
/// that documents an older tsk wrote to the old name are found and merged (ADR 0035).
pub fn request_full_check() {
    FULL_CHECK.store(true, Ordering::SeqCst);
}

pub fn full_check_requested() -> bool {
    FULL_CHECK.load(Ordering::SeqCst)
}

/// `tsk ledger push` asks for the move to be tried even when it failed less than an hour ago.
pub fn request_retry() {
    RETRY_NOW.store(true, Ordering::SeqCst);
}

/// The layouts a nexus ledger is at, for the data ref and the documents ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layouts {
    pub data: RefLayout,
    pub docs: RefLayout,
}

impl Layouts {
    pub const NEW: Layouts = Layouts {
        data: RefLayout::V2,
        docs: RefLayout::V2,
    };
    pub const OLD: Layouts = Layouts {
        data: RefLayout::V1,
        docs: RefLayout::V1,
    };
}

/// What `ls-remote` showed for one pair of refs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairState {
    pub new: Option<String>,
    pub old: Option<String>,
    pub backup: Option<String>,
}

/// What to do with one pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Moved: only the new ref exists.
    Settled,
    /// The new ref exists with a backup beside it: delete the backup when the new ref holds it.
    DropBackup,
    /// The old ref only: move it.
    Move,
    /// The backup only: put it at the new name.
    Resume,
    /// Nothing exists for this pair.
    Nothing,
    /// The old ref and the new one both exist. For the documents ref, an older tsk
    /// recreated the old name after the move.
    Stray,
    /// The old ref exists and so does a backup that holds another commit: something recreated
    /// the old ref after the move deleted it. Nothing is moved.
    Fork,
    /// The data pair holds the old and the new ref: no real server allows it.
    Impossible,
}

/// The decision table of the design, for one pair. `is_data` marks the data pair.
pub fn decide(state: &PairState, is_data: bool) -> Action {
    match (&state.new, &state.old, &state.backup) {
        (Some(_), None, None) => Action::Settled,
        (Some(_), None, Some(_)) => Action::DropBackup,
        (Some(_), Some(_), _) => {
            if is_data {
                Action::Impossible
            } else {
                Action::Stray
            }
        }
        (None, Some(_), None) => Action::Move,
        (None, Some(old), Some(backup)) => {
            if old == backup {
                Action::Move
            } else {
                Action::Fork
            }
        }
        (None, None, Some(_)) => Action::Resume,
        (None, None, None) => Action::Nothing,
    }
}

struct Names {
    ledger: [String; 3],
    docs: [String; 3],
}

impl Names {
    /// New, old and backup names, in `refs` mode.
    fn of(base: &LedgerLocation, refs: LedgerRefs) -> Names {
        let located = base.with_refs(refs, base.source());
        let current = located.with_layout(RefLayout::V2);
        let old = located.legacy_ref_names().expect("a nexus location");
        let backup = current.backup_ref_names().expect("a nexus location");
        Names {
            ledger: [current.ref_name().to_string(), old.0, backup.0],
            docs: [current.docs_ref_name(), old.1, backup.1],
        }
    }

    fn all(&self) -> impl Iterator<Item = &String> {
        self.ledger.iter().chain(self.docs.iter())
    }
}

fn pair_state(listing: &HashMap<String, String>, names: &[String; 3]) -> PairState {
    PairState {
        new: listing.get(&names[0]).cloned(),
        old: listing.get(&names[1]).cloned(),
        backup: listing.get(&names[2]).cloned(),
    }
}

fn list_refs(git: &Git, remote: &str, id: &str) -> Result<HashMap<String, String>, String> {
    let mut args: Vec<String> = vec!["ls-remote".to_string(), remote.to_string()];
    for prefix in [
        LedgerRefs::Branches.nexus_prefix(),
        LedgerRefs::Custom.nexus_prefix(),
        backup_prefix(LedgerRefs::Branches),
        backup_prefix(LedgerRefs::Custom),
    ] {
        args.push(format!("{}{}", prefix, id));
        args.push(format!("{}{}/*", prefix, id));
        args.push(format!("{}{}-docs", prefix, id));
    }
    let (args, output) = git.output(args)?;
    if !output.status.success() {
        return Err(super::git::failure_message(&args, &output));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let sha = fields.next()?;
            let name = fields.next()?;
            Some((name.to_string(), sha.to_string()))
        })
        .collect())
}

pub(crate) fn stale_refs(
    git: &Git,
    target: &LedgerLocation,
) -> Result<Vec<(String, String)>, String> {
    let Some(id) = target.repo_id() else {
        return Ok(Vec::new());
    };
    let held = list_refs(git, target.remote(), id)?;
    let mut names: Vec<String> = Vec::new();
    if let Some((data, docs)) = target.legacy_ref_names() {
        names.extend([data, docs]);
    }
    if let Some((data, docs)) = target.backup_ref_names() {
        names.extend([data, docs]);
    }
    let mut stale: Vec<(String, String)> = names
        .into_iter()
        .filter_map(|name| held.get(&name).map(|sha| (name, sha.clone())))
        .collect();
    stale.sort();
    Ok(stale)
}

/// The layouts to use for this process, and the move when one is needed. `recorded` is the
/// layout the local store was made for, when there is a store.
///
/// Reads the remote once. With nothing to move it writes nothing. A failure that leaves the
/// move undone is a note, not an error: the ledger works on the names it has.
pub fn run(
    repo: &Repo,
    state_root: &Path,
    base: &LedgerLocation,
    recorded: Option<RefLayout>,
) -> Result<Layouts, String> {
    let Some(id) = base.repo_id() else {
        return Ok(Layouts::NEW);
    };
    if let Some(wait) = recently_failed(repo, state_root, base) {
        eprintln!(
            "note: the move of this ledger to the new ref names failed {} minutes ago and is tried again at the next tsk ledger push or in an hour",
            wait
        );
        return Ok(match recorded {
            Some(RefLayout::V2) => Layouts::NEW,
            _ => Layouts::OLD,
        });
    }
    // Another process of this home may be in the middle of the move: wait for it, then read.
    // Not while the local move of this clone is still to run: that move takes `migrate.lock`
    // and the old push lock before the home lock, so the home lock is not taken first here.
    let early_lock = match peek_home(repo, state_root, base) {
        Some(existing) if existing.root().exists() && !has_legacy_state(repo, state_root) => {
            Some(clone_lock::take(&existing.db())?)
        }
        _ => None,
    };
    let git = super::nexus::open_clone(state_root)?;
    let listing = match list_refs(&git, base.remote(), id) {
        Ok(listing) => listing,
        // The remote cannot be asked: the layout the store was made for stands.
        Err(_) => {
            return Ok(match recorded {
                Some(RefLayout::V1) => Layouts::OLD,
                _ => Layouts::NEW,
            })
        }
    };
    let mode = base.refs();
    let this = Names::of(base, mode);
    let other = Names::of(base, mode.other());
    let holds = |names: &Names| names.all().any(|n| listing.contains_key(n));
    if holds(&other) && !holds(&this) {
        // The ledger is on the other refs: the mismatch error says so, and nothing is written.
        return Ok(Layouts::NEW);
    }

    let data = pair_state(&listing, &this.ledger);
    let docs = pair_state(&listing, &this.docs);
    let mut data_action = decide(&data, true);
    let mut docs_action = decide(&docs, false);
    // A backup that the old ref has moved past is stale, not a fork: the ledger stayed on the
    // old names after an earlier move did not finish, and its work went on there.
    if let Action::Fork = data_action {
        if backup_is_behind(&git, base.remote(), &this.ledger, PIN_LEDGER) {
            data_action = Action::Move;
        }
    }
    if let Action::Fork = docs_action {
        if backup_is_behind(&git, base.remote(), &this.docs, PIN_DOCS) {
            docs_action = Action::Move;
        }
    }
    if let Action::Fork = data_action {
        return Err(fork_error(base, &this.ledger, &data));
    }
    if let Action::Fork = docs_action {
        return Err(fork_error(base, &this.docs, &docs));
    }
    if let Action::Impossible = data_action {
        return Err(format!(
            "error: {} holds both {} and {}, which git cannot hold together; the nexus is damaged",
            base.label(),
            this.ledger[0],
            this.ledger[1]
        ));
    }
    let quiet = |action: &Action| matches!(action, Action::Settled | Action::Nothing);
    if quiet(&data_action) && quiet(&docs_action) {
        clear_last(repo, state_root, base);
        return Ok(Layouts::NEW);
    }

    let home = home::for_location(repo, state_root, base)?;
    let _lock = match early_lock {
        Some(lock) => lock,
        None => {
            let lock = clone_lock::take(&home.db())?;
            // Taken late: another process may have moved the refs since the read.
            drop(lock);
            return run(repo, state_root, base, recorded);
        }
    };
    let mover = Mover {
        base,
        home: &home,
        bare: home.git(),
        remote: base.remote().to_string(),
    };
    let mut layouts = Layouts::NEW;
    let mut failure: Option<String> = None;
    let mut moved = false;

    // The data pair first: the documents pair is never moved ahead of it.
    match mover.pair(PIN_LEDGER, &this.ledger, &data, &data_action) {
        Ok(Done::Moved) => moved = true,
        Ok(Done::Unchanged) => {}
        Ok(Done::Stayed(why)) => {
            layouts.data = RefLayout::V1;
            failure = Some(why);
        }
        Err(e) => {
            layouts.data = RefLayout::V1;
            failure = Some(e);
        }
    }
    if matches!(data_action, Action::Nothing) {
        layouts.data = RefLayout::V2;
    }
    match mover.pair(PIN_DOCS, &this.docs, &docs, &docs_action) {
        Ok(Done::Moved) => moved = true,
        Ok(Done::Unchanged) => {}
        Ok(Done::Stayed(why)) => {
            layouts.docs = RefLayout::V1;
            failure.get_or_insert(why);
        }
        Err(e) => {
            layouts.docs = RefLayout::V1;
            failure.get_or_insert(e);
        }
    }
    if matches!(docs_action, Action::Nothing) {
        layouts.docs = RefLayout::V2;
    }

    match failure {
        None => {
            clear_last(repo, state_root, base);
            if moved && layouts == Layouts::NEW {
                // The marker comes last: an older tsk keeps working on the old names until
                // the move is whole.
                mark_entry(state_root, base, id);
            }
        }
        Some(why) => {
            note_failure(&home, &why);
            eprintln!(
                "note: the ledger of this repo stays on the old ref names for now; nothing was lost.\n      {}",
                why.lines().next().unwrap_or("").trim_start_matches("error: ")
            );
        }
    }
    Ok(layouts)
}

/// True when the backup ref of a pair holds a commit that the old ref contains: the old ref
/// advanced after the backup was made. The two refs are fetched into pins of the scratch clone.
fn backup_is_behind(git: &Git, remote: &str, names: &[String; 3], pin: &str) -> bool {
    let old_pin = format!("{}-fork-old", pin);
    let backup_pin = format!("{}-fork-backup", pin);
    let specs = [
        format!("+{}:{}", names[1], old_pin),
        format!("+{}:{}", names[2], backup_pin),
    ];
    let fetched = git
        .output([
            "fetch",
            "--quiet",
            "--no-tags",
            remote,
            &specs[0],
            &specs[1],
        ])
        .is_ok_and(|(_, out)| out.status.success());
    if !fetched {
        return false;
    }
    let (old, backup) = (
        format!("{}^{{commit}}", old_pin),
        format!("{}^{{commit}}", backup_pin),
    );
    git.output(["merge-base", "--is-ancestor", backup.as_str(), old.as_str()])
        .is_ok_and(|(_, out)| out.status.success())
}

fn fork_error(base: &LedgerLocation, names: &[String; 3], state: &PairState) -> String {
    format!(
        "error: {} holds {} at {} and its backup {} at {}, which differ. Something recreated the old name after the move deleted it.\n       \
         The ledger is the one at the backup. Nothing was moved or deleted; look at both with git, keep the one you want at {}, and run the command again",
        base.label(),
        names[1],
        state.old.as_deref().unwrap_or(""),
        names[2],
        state.backup.as_deref().unwrap_or(""),
        names[0]
    )
}

/// The minutes since a failed move, when it failed less than an hour ago and nothing asked
/// for a retry.
fn recently_failed(repo: &Repo, state_root: &Path, base: &LedgerLocation) -> Option<u64> {
    if RETRY_NOW.load(Ordering::SeqCst) {
        return None;
    }
    let home = peek_home(repo, state_root, base)?;
    let text = std::fs::read_to_string(home.root().join(LAST_FILE)).ok()?;
    let at: u64 = text.lines().next()?.trim().parse().ok()?;
    let age = now_secs().saturating_sub(at);
    (age < RETRY_SECS).then_some(age / 60)
}

/// True when a move failed before and has not finished: the next command tries it again.
pub fn pending(repo: &Repo, state_root: &Path, base: &LedgerLocation) -> bool {
    peek_home(repo, state_root, base).is_some_and(|h| h.root().join(LAST_FILE).exists())
}

/// True when this clone still holds the ledger state of tsk 0.69.0 and earlier.
fn has_legacy_state(repo: &Repo, state_root: &Path) -> bool {
    repo.read_clone_id().ok().flatten().is_some_and(|id| {
        super::migrate_home::has_legacy_state(&home::clone_state_dir(state_root, &id))
    })
}

fn peek_home(repo: &Repo, state_root: &Path, base: &LedgerLocation) -> Option<Home> {
    let id = base.repo_id()?;
    let named = home::peek(repo, state_root).ok()?;
    match named {
        home::Opened::Home(h) if h.root().exists() => Some(h),
        _ => {
            let h = Home::of_key(state_root, home::HomeKey::Nexus(id.to_string()));
            h.root().exists().then_some(h)
        }
    }
}

fn clear_last(repo: &Repo, state_root: &Path, base: &LedgerLocation) {
    if let Some(home) = peek_home(repo, state_root, base) {
        let _ = std::fs::remove_file(home.root().join(LAST_FILE));
    }
}

fn note_failure(home: &Home, why: &str) {
    let _ = std::fs::write(
        home.root().join(LAST_FILE),
        format!("{}\n{}\n", now_secs(), why),
    );
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Mark the nexus entry as moved. A failure is a note: the server rule keeps an older tsk
/// from forking the ledger, and the next publish point tries again.
fn mark_entry(state_root: &Path, base: &LedgerLocation, id: &str) {
    if let Err(e) = super::register::set_entry_marker(state_root, base.remote(), id, base.refs()) {
        eprintln!(
            "note: could not mark the nexus entry \"{}\" as moved to the new ref names; it is marked at the next publish: {}",
            id,
            e.lines().next().unwrap_or("").trim_start_matches("error: ")
        );
    }
}

/// What the move of one pair did.
enum Done {
    Moved,
    Unchanged,
    /// The move did not happen, and the pair stays on its old names.
    Stayed(String),
}

struct Mover<'a> {
    base: &'a LedgerLocation,
    home: &'a Home,
    bare: Git,
    remote: String,
}

impl Mover<'_> {
    fn pair(
        &self,
        pin: &str,
        names: &[String; 3],
        state: &PairState,
        action: &Action,
    ) -> Result<Done, String> {
        match action {
            Action::Settled | Action::Nothing => Ok(Done::Unchanged),
            Action::DropBackup => {
                self.drop_backup_if_contained(pin, names, state.backup.as_deref().unwrap_or(""))?;
                Ok(Done::Unchanged)
            }
            Action::Move => self.move_pair(pin, names, state),
            Action::Resume => self.resume_pair(pin, names, state),
            Action::Stray => self.merge_stray(names, state),
            Action::Fork | Action::Impossible => Ok(Done::Unchanged),
        }
    }

    fn fetch_pin(&self, pin: &str, source: &str) -> Result<String, String> {
        let spec = format!("+{}:{}", source, pin);
        self.bare.run([
            "fetch",
            "--quiet",
            "--no-tags",
            self.remote.as_str(),
            spec.as_str(),
        ])?;
        let commit = format!("{}^{{commit}}", pin);
        self.bare
            .run_line(["rev-parse", "--verify", commit.as_str()])
    }

    fn push(&self, args: Vec<String>) -> Result<(Vec<String>, std::process::Output), String> {
        let mut all = vec!["push".to_string(), "--porcelain".to_string()];
        all.extend(args);
        self.bare.output(all)
    }

    fn move_pair(&self, pin: &str, names: &[String; 3], state: &PairState) -> Result<Done, String> {
        let [new, old, backup] = names;
        let tip = self.fetch_pin(pin, old)?;
        // The backup first: from here on one ref on the remote always holds the tip.
        if state.backup.as_deref() != Some(tip.as_str()) {
            // No backup yet, or a stale one that the old ref has moved past: the backup goes
            // forward to the tip, under a lease on what it held.
            let spec = format!("{}:{}", tip, backup);
            let mut args = Vec::new();
            if let Some(held) = &state.backup {
                args.push(format!("--force-with-lease={}:{}", backup, held));
            }
            args.extend([self.remote.clone(), spec]);
            let (args, output) = self.push(args)?;
            if !output.status.success() {
                return Err(super::git::failure_message(&args, &output));
            }
        }
        let lease = format!("--force-with-lease={}:{}", old, tip);
        let delete = format!(":{}", old);
        let create = format!("{}:{}", tip, new);
        let (args, output) = self.push(vec![
            lease.clone(),
            self.remote.clone(),
            delete.clone(),
            create.clone(),
        ])?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let deleted = parse_push_status(&stdout, old);
        let created = parse_push_status(&stdout, new);
        let ok = |s: &Option<super::push::PushStatus>| s.as_ref().is_some_and(|s| !s.rejected());
        if ok(&deleted) && ok(&created) {
            return self.finish(pin, names, &tip);
        }
        let lost_lease = |s: &Option<super::push::PushStatus>| {
            s.as_ref()
                .is_some_and(|s| s.rejected() && s.summary.contains("stale info"))
        };
        if lost_lease(&deleted) {
            // Another machine moved the pair between the read and the push.
            return Ok(Done::Stayed(format!(
                "{} changed while the move ran; it is tried again at the next push",
                old
            )));
        }
        // The server refused the combined push for another reason: the delete and the create,
        // one after the other.
        let failure = super::git::failure_message(&args, &output);
        self.fallback(pin, names, &tip, &failure)
    }

    fn fallback(
        &self,
        pin: &str,
        names: &[String; 3],
        tip: &str,
        first_failure: &str,
    ) -> Result<Done, String> {
        let [new, old, _] = names;
        // The combined push may have done half of its work: read the pair again.
        let now = pair_state(
            &list_refs(&self.bare, &self.remote, self.base.repo_id().unwrap_or(""))?,
            names,
        );
        if now.new.is_some() {
            return self.finish(pin, names, tip);
        }
        if now.old.is_some() {
            let lease = format!("--force-with-lease={}:{}", old, tip);
            let (args, output) =
                self.push(vec![lease, self.remote.clone(), format!(":{}", old)])?;
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let status = parse_push_status(&stdout, old);
            let deleted = status.as_ref().is_some_and(|s| !s.rejected()) && output.status.success();
            if !deleted {
                let why = super::git::failure_message(&args, &output);
                // The server refused the delete: the old ref and the backup stand, and the
                // ledger keeps working on the old names. The backup has served its turn.
                self.try_delete_backup(names, tip);
                return Ok(Done::Stayed(format!(
                    "the nexus refused to delete {}: {}",
                    old,
                    last_line(&why, first_failure)
                )));
            }
        }
        let spec = format!("{}:{}", tip, new);
        let mut last = String::new();
        for _ in 0..3 {
            let (args, output) = self.push(vec![self.remote.clone(), spec.clone()])?;
            if output.status.success() {
                return self.finish(pin, names, tip);
            }
            last = super::git::failure_message(&args, &output);
            std::thread::sleep(super::transport::retry_wait());
        }
        // The old ref is gone and the new one could not be made: the backup holds the tip, and
        // the next command resumes from it.
        Err(format!(
            "could not create {} after deleting {}; the backup {} holds the ledger and the next command resumes: {}",
            new,
            old,
            names[2],
            last_line(&last, "")
        ))
    }

    /// The new ref exists: take the backup away once the new ref holds its commit.
    fn finish(&self, pin: &str, names: &[String; 3], tip: &str) -> Result<Done, String> {
        self.drop_backup_if_contained(pin, names, tip)?;
        Ok(Done::Moved)
    }

    fn drop_backup_if_contained(
        &self,
        pin: &str,
        names: &[String; 3],
        backup_tip: &str,
    ) -> Result<(), String> {
        let new_pin = format!("{}-new", pin);
        let new_tip = self.fetch_pin(&new_pin, &names[0])?;
        let contained = new_tip == backup_tip
            || self
                .bare
                .output(["merge-base", "--is-ancestor", backup_tip, new_tip.as_str()])
                .is_ok_and(|(_, out)| out.status.success());
        if !contained {
            return Err(format!(
                "error: {} at {} does not contain the backup {} at {}; the backup is kept",
                names[0], new_tip, names[2], backup_tip
            ));
        }
        self.try_delete_backup(names, backup_tip);
        Ok(())
    }

    fn try_delete_backup(&self, names: &[String; 3], tip: &str) {
        let lease = format!("--force-with-lease={}:{}", names[2], tip);
        let _ = self.push(vec![lease, self.remote.clone(), format!(":{}", names[2])]);
    }

    /// The old ref is gone and the backup is the only copy: put it at the new name.
    fn resume_pair(
        &self,
        pin: &str,
        names: &[String; 3],
        state: &PairState,
    ) -> Result<Done, String> {
        let [new, _, backup] = names;
        let tip = self.fetch_pin(pin, backup)?;
        let spec = format!("{}:{}", tip, new);
        let (args, output) = self.push(vec![self.remote.clone(), spec])?;
        if !output.status.success() {
            // Another machine created the new ref first: theirs stands when it holds the
            // backup, else the two differ and nothing is deleted.
            let listing = list_refs(&self.bare, &self.remote, self.base.repo_id().unwrap_or(""))?;
            if listing.contains_key(new) {
                self.drop_backup_if_contained(pin, names, state.backup.as_deref().unwrap_or(&tip))?;
                return Ok(Done::Unchanged);
            }
            return Err(super::git::failure_message(&args, &output));
        }
        self.finish(pin, names, &tip)
    }

    /// An older tsk recreated the old documents name after the move. Its documents are real
    /// work: the commit is merged into the new documents ref, then the old ref goes.
    fn merge_stray(&self, names: &[String; 3], state: &PairState) -> Result<Done, String> {
        let [new, old, _] = names;
        let old_tip = self.fetch_pin("refs/tsk/migrate/docs-stray", old)?;
        let new_tip = self.fetch_pin("refs/tsk/migrate/docs-new", new)?;
        let contained = old_tip == new_tip
            || self
                .bare
                .output([
                    "merge-base",
                    "--is-ancestor",
                    old_tip.as_str(),
                    new_tip.as_str(),
                ])
                .is_ok_and(|(_, out)| out.status.success());
        let delete_old = |tip: &str| -> Result<(), String> {
            let lease = format!("--force-with-lease={}:{}", old, tip);
            let (args, output) =
                self.push(vec![lease, self.remote.clone(), format!(":{}", old)])?;
            if output.status.success() {
                Ok(())
            } else {
                Err(super::git::failure_message(&args, &output))
            }
        };
        if contained {
            delete_old(&old_tip)?;
            return Ok(Done::Moved);
        }
        let temp = self.home.root().join("docs.stray");
        if temp.exists() {
            let _ = self
                .bare
                .output(["worktree", "remove", "--force", &temp.to_string_lossy()]);
            let _ = std::fs::remove_dir_all(&temp);
        }
        let _ = self.bare.output(["worktree", "prune"]);
        self.bare.run([
            "worktree",
            "add",
            "--detach",
            "--quiet",
            &temp.to_string_lossy(),
            new_tip.as_str(),
        ])?;
        let wt = Git::new(&temp);
        let merged = (|| -> Result<(), String> {
            let (git, mut args) = crate::attribution::ledger_git(&wt)?;
            args.extend(
                [
                    "merge",
                    "--no-edit",
                    "--quiet",
                    "-m",
                    "Merge the documents that an older tsk wrote to the old name",
                    old_tip.as_str(),
                ]
                .map(String::from),
            );
            let (_, output) = git.output(args)?;
            if !output.status.success() {
                let _ = wt.output(["merge", "--abort"]);
                return Err("conflict".to_string());
            }
            let head = wt.run_line(["rev-parse", "HEAD"])?;
            let lease = format!("--force-with-lease={}:{}", new, new_tip);
            let spec = format!("{}:{}", head, new);
            let (args, output) = self.push(vec![lease, self.remote.clone(), spec])?;
            if !output.status.success() {
                return Err(super::git::failure_message(&args, &output));
            }
            Ok(())
        })();
        let _ = self
            .bare
            .output(["worktree", "remove", "--force", &temp.to_string_lossy()]);
        let _ = std::fs::remove_dir_all(&temp);
        let _ = self.bare.output(["worktree", "prune"]);
        match merged {
            Ok(()) => {
                delete_old(&old_tip)?;
                Ok(Done::Moved)
            }
            Err(why) if why == "conflict" => {
                eprintln!(
                    "note: documents that an older tsk wrote to {} conflict with {}. They are left at {} ({}); look at them with git and merge by hand",
                    old, new, old, state.old.as_deref().unwrap_or("")
                );
                Ok(Done::Unchanged)
            }
            Err(why) => Err(why),
        }
    }
}

/// The last line of a failure, or `fallback` when it has none.
fn last_line(text: &str, fallback: &str) -> String {
    text.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(new: Option<&str>, old: Option<&str>, backup: Option<&str>) -> PairState {
        let own = |s: Option<&str>| s.map(str::to_string);
        PairState {
            new: own(new),
            old: own(old),
            backup: own(backup),
        }
    }

    #[test]
    fn the_table_decides_every_combination() {
        let t = Some("t");
        assert_eq!(decide(&state(t, None, None), true), Action::Settled);
        assert_eq!(decide(&state(t, None, t), true), Action::DropBackup);
        assert_eq!(decide(&state(t, t, None), true), Action::Impossible);
        assert_eq!(decide(&state(t, t, None), false), Action::Stray);
        assert_eq!(decide(&state(None, t, None), true), Action::Move);
        assert_eq!(decide(&state(None, t, t), true), Action::Move);
        assert_eq!(
            decide(&state(None, Some("a"), Some("b")), true),
            Action::Fork
        );
        assert_eq!(decide(&state(None, None, t), true), Action::Resume);
        assert_eq!(decide(&state(None, None, None), true), Action::Nothing);
    }

    #[test]
    fn the_names_of_a_pair_are_new_old_and_backup_in_one_mode() {
        let base = LedgerLocation::nexus("n", "work-api");
        let names = Names::of(&base, LedgerRefs::Custom);
        assert_eq!(
            names.ledger,
            [
                "refs/tsk/ledgers/work-api/ledger".to_string(),
                "refs/tsk/ledgers/work-api".to_string(),
                "refs/tsk/ledgers-backup/work-api/ledger".to_string()
            ]
        );
        assert_eq!(
            names.docs,
            [
                "refs/tsk/ledgers/work-api/docs".to_string(),
                "refs/tsk/ledgers/work-api-docs".to_string(),
                "refs/tsk/ledgers-backup/work-api/docs".to_string()
            ]
        );
        let branches = Names::of(&base, LedgerRefs::Branches);
        assert_eq!(branches.ledger[1], "refs/heads/ledgers/work-api");
    }
}
