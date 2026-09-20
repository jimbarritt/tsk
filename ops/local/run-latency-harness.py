#!/usr/bin/env python3
"""Measure per-turn latency across plugin and output-style conditions.

Runs `claude -p` headless, one fresh session per run, and reads the timings
back out of the CLI's own result JSON and the session transcript. No
stopwatch: every number here comes from a field the harness did not compute.

Design reference: ops/local/run-latency-harness.md
"""

import argparse
import json
import os
import pathlib
import random
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
import uuid

PROJECTS_DIR = pathlib.Path.home() / ".claude" / "projects"
DEFAULT_PLUGIN_CACHE = (
    pathlib.Path.home() / ".claude" / "plugins" / "cache" / "jimbarritt-claude-plugins" / "swe"
)

# Three target lengths, none needing a tool call. Output length drives
# latency, so the comparison holds only when the prompt is identical across
# conditions.
PROMPTS = {
    "short": "Reply with one short sentence: what is a git worktree?",
    "medium": (
        "In about 150 words, explain what a git worktree is, how it differs "
        "from a branch, and when to use one. Prose only, no headings, no "
        "code blocks."
    ),
    "long": (
        "In about 600 words, explain how git stores history: blobs, trees, "
        "commits, refs, and how a branch differs from a tag. Cover why a "
        "commit hash cannot be known before the commit is written, and what "
        "a detached HEAD is. Prose only, no headings, no code blocks."
    ),
}

CONDITION_HELP = {
    "A": "no plugin, default output style",
    "B": "plugin loaded with its output style removed, default output style",
    "C": "plugin loaded intact, so its forced output style applies",
}


def resolve_plugin_root(given):
    """The plugin directory to load. Newest version under the cache by default."""
    if given:
        root = pathlib.Path(given)
        if not root.is_dir():
            sys.exit(f"error: --plugin-root {root} is not a directory")
        return root
    if not DEFAULT_PLUGIN_CACHE.is_dir():
        sys.exit(
            f"error: no plugin cache at {DEFAULT_PLUGIN_CACHE}. "
            "Pass --plugin-root, or install the plugin first."
        )
    versions = sorted(p for p in DEFAULT_PLUGIN_CACHE.iterdir() if p.is_dir())
    if not versions:
        sys.exit(f"error: no versions under {DEFAULT_PLUGIN_CACHE}")
    return versions[-1]


def stage_plugin_without_output_styles(plugin_root, staging_dir):
    """Copy the plugin and drop its output-styles directory.

    The plugin's own output style carries `force-for-plugin: true`, so loading
    the plugin applies that style whatever the session's outputStyle setting
    says. A condition that wants the plugin's hooks without its style is
    therefore not reachable through settings alone. Removing the directory
    from a copy separates the two factors. The copy is temporary and the
    installed plugin is never touched.
    """
    dest = staging_dir / f"{plugin_root.name}-no-output-style"
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(plugin_root, dest)
    styles = dest / "output-styles"
    if styles.is_dir():
        shutil.rmtree(styles)
    return dest


def build_args(condition, plugin_root, staged_root, model, session_id, prompt):
    args = [
        "claude",
        "-p",
        prompt,
        "--model",
        model,
        "--session-id",
        session_id,
        "--output-format",
        "json",
        # User settings only. Project settings enable the plugin and register
        # this repo's own hooks, both of which would vary the thing under
        # test. User settings hold the model and auth configuration and are
        # constant across conditions.
        "--setting-sources",
        "user",
    ]
    if condition == "B":
        args += ["--plugin-dir", str(staged_root)]
    elif condition == "C":
        args += ["--plugin-dir", str(plugin_root)]
    return args


def find_transcript(session_id):
    """The session's JSONL, wherever the working directory put it."""
    matches = list(PROJECTS_DIR.glob(f"*/{session_id}.jsonl"))
    return matches[0] if matches else None


def read_hook_costs(session_id):
    """Per-hook durations the CLI recorded for this session.

    The result JSON carries no hook timing. The transcript does, under
    stop_hook_summary entries, one durationMs per hook command.
    """
    path = find_transcript(session_id)
    if path is None:
        return [], None
    totals = {}
    with path.open() as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            for info in entry.get("hookInfos") or []:
                command = info.get("command", "unknown")
                totals[command] = totals.get(command, 0) + int(info.get("durationMs") or 0)
    hooks = [{"command": k, "ms": v} for k, v in sorted(totals.items())]
    return hooks, str(path)


def run_once(condition, prompt_key, repeat, plugin_root, staged_root, model, workdir, timeout):
    session_id = str(uuid.uuid4())
    args = build_args(condition, plugin_root, staged_root, model, session_id, PROMPTS[prompt_key])
    started = time.time()
    try:
        proc = subprocess.run(
            args, cwd=workdir, capture_output=True, text=True, timeout=timeout
        )
    except subprocess.TimeoutExpired:
        return {
            "condition": condition,
            "prompt_key": prompt_key,
            "repeat": repeat,
            "model": model,
            "session_id": session_id,
            "is_error": True,
            "error": f"timed out after {timeout}s",
            "wall_ms": int((time.time() - started) * 1000),
        }
    wall_ms = int((time.time() - started) * 1000)

    row = {
        "condition": condition,
        "prompt_key": prompt_key,
        "repeat": repeat,
        "model": model,
        "session_id": session_id,
        "wall_ms": wall_ms,
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(started)),
    }

    try:
        result = json.loads(proc.stdout)
    except json.JSONDecodeError:
        row["is_error"] = True
        row["error"] = (proc.stderr or proc.stdout or "no output")[:500]
        return row

    usage = result.get("usage") or {}
    output_tokens = usage.get("output_tokens") or 0
    duration_ms = result.get("duration_ms")
    hooks, transcript = read_hook_costs(session_id)

    row.update(
        {
            "is_error": bool(result.get("is_error")),
            "duration_ms": duration_ms,
            "ttft_ms": result.get("ttft_ms"),
            "duration_api_ms": result.get("duration_api_ms"),
            "output_tokens": output_tokens,
            "thinking_tokens": (usage.get("output_tokens_details") or {}).get("thinking_tokens"),
            "total_cost_usd": result.get("total_cost_usd"),
            "num_turns": result.get("num_turns"),
            "hook_ms_total": sum(h["ms"] for h in hooks),
            "hooks": hooks,
            "transcript": transcript,
            "ms_per_output_token": (duration_ms / output_tokens)
            if duration_ms and output_tokens
            else None,
            # Kept so a reader can confirm the condition did what it claims:
            # condition C's replies follow Software English, B's do not.
            "result_head": (result.get("result") or "")[:400],
        }
    )
    return row


def summarise(rows, key):
    """Median, mean and spread of one field, per condition and prompt length."""
    table = {}
    for row in rows:
        if row.get("is_error") or row.get(key) is None:
            continue
        table.setdefault((row["condition"], row["prompt_key"]), []).append(row[key])
    lines = []
    for (condition, prompt_key), values in sorted(table.items()):
        lines.append(
            f"  {condition}  {prompt_key:<7} n={len(values):<3} "
            f"median={statistics.median(values):>10.1f}  "
            f"mean={statistics.fmean(values):>10.1f}  "
            f"min={min(values):>10.1f}  max={max(values):>10.1f}"
        )
    return lines


def main():
    parser = argparse.ArgumentParser(
        description="Measure per-turn latency across plugin and output-style conditions.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="\n".join(f"  {k}: {v}" for k, v in CONDITION_HELP.items()),
    )
    parser.add_argument("--repeats", type=int, default=5, help="runs per cell (default 5)")
    parser.add_argument("--model", default="sonnet", help="model alias or id (default sonnet)")
    parser.add_argument(
        "--conditions", default="A,B,C", help="comma-separated subset of A,B,C (default all)"
    )
    parser.add_argument(
        "--lengths",
        default="short,medium,long",
        help="comma-separated subset of short,medium,long (default all)",
    )
    parser.add_argument("--out", default="latency-results.ndjson", help="results file to write")
    parser.add_argument("--plugin-root", default=None, help="plugin directory to load")
    parser.add_argument("--timeout", type=int, default=300, help="per-run timeout in seconds")
    parser.add_argument("--seed", type=int, default=0, help="shuffle seed for run order")
    parser.add_argument(
        "--no-warmup",
        action="store_true",
        help="skip the discarded warm-up run per condition",
    )
    parser.add_argument(
        "--dry-run", action="store_true", help="print the plan and the commands, run nothing"
    )
    opts = parser.parse_args()

    conditions = [c.strip().upper() for c in opts.conditions.split(",") if c.strip()]
    lengths = [l.strip() for l in opts.lengths.split(",") if l.strip()]
    for condition in conditions:
        if condition not in CONDITION_HELP:
            sys.exit(f"error: unknown condition {condition}")
    for length in lengths:
        if length not in PROMPTS:
            sys.exit(f"error: unknown length {length}")

    plugin_root = resolve_plugin_root(opts.plugin_root) if conditions != ["A"] else None

    plan = [
        (condition, length, repeat)
        for condition in conditions
        for length in lengths
        for repeat in range(1, opts.repeats + 1)
    ]
    # Shuffle so a slow period in the API does not land entirely on one
    # condition. Running every A first and every C last would read as a
    # difference between conditions when it is a difference in time.
    random.Random(opts.seed).shuffle(plan)

    print(f"conditions: {', '.join(conditions)}")
    print(f"lengths:    {', '.join(lengths)}")
    print(f"repeats:    {opts.repeats}  ->  {len(plan)} measured runs")
    print(f"model:      {opts.model}")
    if plugin_root:
        print(f"plugin:     {plugin_root}")

    with tempfile.TemporaryDirectory(prefix="tsk-latency-") as tmp:
        tmp_path = pathlib.Path(tmp)
        workdir = tmp_path / "cwd"
        workdir.mkdir()
        staged_root = None
        if "B" in conditions:
            staged_root = stage_plugin_without_output_styles(plugin_root, tmp_path)
            print(f"staged B:   {staged_root} (output-styles removed)")

        if opts.dry_run:
            for condition, length, repeat in plan:
                args = build_args(
                    condition, plugin_root, staged_root, opts.model, "<uuid>", PROMPTS[length]
                )
                print(f"\n{condition} {length} #{repeat}:\n  {' '.join(args[:1] + args[2:])}")
            return

        rows = []
        if not opts.no_warmup:
            for condition in conditions:
                print(f"warm-up {condition} (discarded) ...", flush=True)
                run_once(
                    condition, lengths[0], 0, plugin_root, staged_root,
                    opts.model, workdir, opts.timeout,
                )

        out_path = pathlib.Path(opts.out)
        with out_path.open("w") as handle:
            for index, (condition, length, repeat) in enumerate(plan, start=1):
                print(f"[{index}/{len(plan)}] {condition} {length} #{repeat} ...", end="", flush=True)
                row = run_once(
                    condition, length, repeat, plugin_root, staged_root,
                    opts.model, workdir, opts.timeout,
                )
                rows.append(row)
                handle.write(json.dumps(row) + "\n")
                handle.flush()
                if row.get("is_error"):
                    print(f" ERROR: {row.get('error', 'see results file')}")
                else:
                    print(
                        f" {row['duration_ms']}ms"
                        f"  ttft {row['ttft_ms']}ms"
                        f"  out {row['output_tokens']}tok"
                        f"  think {row['thinking_tokens']}tok"
                        f"  hooks {row['hook_ms_total']}ms"
                    )

    errors = [r for r in rows if r.get("is_error")]
    print(f"\nwrote {len(rows)} rows to {opts.out} ({len(errors)} errors)")
    for field in ("duration_ms", "ttft_ms", "output_tokens", "thinking_tokens", "hook_ms_total"):
        print(f"\n{field}:")
        for line in summarise(rows, field):
            print(line)


if __name__ == "__main__":
    main()
