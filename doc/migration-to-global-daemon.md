# Migration guide: per-project daemon → global daemon

This guide covers migrating from the old per-project `tskd` model to the new single
global daemon introduced in tsk 1.6+.

## What changed

| Old model | New model |
|-----------|-----------|
| One `tskd` per project, started in the project root | One `tskd` per user, started anywhere |
| Socket at `/tmp/tsk-{project-hash}.sock` | Socket at `~/.tsk/tskd.sock` |
| State in `{project}/tsk/` (committed to source control) | State in `~/.tsk/` (user home) |
| `TSK_PROJECT_ROOT` environment variable | `TSK_HOME` environment variable (optional override) |

Threads can still be associated with a project directory via an optional `path` field —
see step 4.

---

## Migration steps

### 1. Stop all running `tskd` instances

```bash
pkill tskd
```

Verify no daemons are running:

```bash
pgrep tskd   # should return nothing
```

### 2. Create `~/.tsk/` and its subdirectories

```bash
mkdir -p ~/.tsk/event-log ~/.tsk/threads
```

### 3. Migrate state from each project

For each project that had a `tsk/` directory, run the following steps.

Replace `<project>` with the absolute path to the project root (e.g. `~/projects/my-app`).

#### a. Copy thread directories

```bash
cp -r <project>/tsk/threads/0*  ~/.tsk/threads/
```

This copies per-thread directories (e.g. `0001-fix-login/`) preserving their contents.

**Note:** if you are migrating multiple projects and thread ids collide (e.g. both have a
`0001-something/`), you will need to manually renumber threads in `~/.tsk/threads/index.json`
after merging. See step c.

#### b. Merge the event log

```bash
cat <project>/tsk/event-log/events.ndjson >> ~/.tsk/event-log/events.ndjson
```

If `~/.tsk/event-log/events.ndjson` does not yet exist, copy instead:

```bash
cp <project>/tsk/event-log/events.ndjson ~/.tsk/event-log/events.ndjson
```

#### c. Merge `index.json`

The authoritative thread state is in `index.json`. If you are migrating a single project:

```bash
cp <project>/tsk/threads/index.json ~/.tsk/threads/index.json
```

If you are migrating multiple projects, you need to merge the arrays manually. Each
`index.json` is a JSON array of thread objects. Combine them into one array and ensure
`id` values are unique across all threads. A quick approach with `jq`:

```bash
# Merge two index.json files (adjust paths as needed)
jq -s 'add' \
  <project-a>/tsk/threads/index.json \
  <project-b>/tsk/threads/index.json \
  > ~/.tsk/threads/index.json
```

If thread ids collide, you will need to renumber threads and rename their directories to
match (e.g. rename `0001-fix-login/` to `0005-fix-login/` and update `id` in `index.json`).

### 4. Start the new global daemon

```bash
tskd &
```

Verify it started:

```bash
ls ~/.tsk/tskd.sock   # socket file should exist
tsk thread list       # should return your migrated threads
```

### 5. Add project path bindings

Threads no longer live inside projects, but they can be **bound** to a project directory
so that `tsk where` works when you are inside that directory.

For each thread that belongs to a specific project:

```bash
tsk thread update <id-or-slug> --path /abs/path/to/project
```

Example:

```bash
tsk thread update fix-login --path ~/projects/my-app
tsk thread update update-deps --path ~/projects/my-app
```

Verify:

```bash
cd ~/projects/my-app
tsk where
```

### 6. (Optional) Set up `doc/tsk/` for project-local context

If you want `tsk` to auto-zoom to the bound thread when run from a project directory,
create a `doc/tsk/` directory in the project:

```bash
mkdir -p <project>/doc/tsk
```

You can commit project-local notes and context files here. The global thread context
(index.md and any other files you created) remains in `~/.tsk/threads/{id}-{slug}/`.

### 7. Remove old `tsk/` directories from projects (optional)

Once you have verified the migration, remove the old per-project state directories:

```bash
rm -rf <project>/tsk
```

You may also want to add `tsk/` to `.gitignore` if it is no longer committed, or remove
the existing `tsk/` entry from source control:

```bash
git rm -r --cached <project>/tsk
```

---

## Verifying the migration

```bash
tsk thread list           # all threads from all migrated projects should appear
tsk where                 # from inside a project, shows bound thread
tsk                       # TUI should show all threads
```

---

## Rollback

If you need to revert to the old model, reinstall an older version of `tsk-bin` and `tskd`
(before 1.6), start `tskd` from your project root as before, and point `TSK_PROJECT_ROOT`
at the project directory if needed.
