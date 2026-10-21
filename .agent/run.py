"""Cartload agent driver: runs task briefs with the Claude Agent SDK, billed to ANTHROPIC_API_KEY.

Usage:
  uv run run.py                      # run every pending brief in tasks/, in name order
  uv run run.py tasks/02-fel.md      # run one or more specific briefs
  uv run run.py --model claude-opus-5-5 --effort high --budget 20 tasks/01-scaffold.md

Each brief runs in a fresh session with cwd = repo root. Output streams to the terminal and to
logs/<task>-<timestamp>.log. Cost per task is appended to logs/costs.tsv. A brief that finishes
without error is moved to tasks/done/.
"""
from __future__ import annotations

import argparse
import asyncio
import datetime as dt
import os
import shutil
import sys
import traceback
from pathlib import Path

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    CLINotFoundError,
    ProcessError,
    ResultMessage,
    SystemMessage,
    TextBlock,
    ToolResultBlock,
    ToolUseBlock,
    UserMessage,
    query,
)

AGENT_DIR = Path(__file__).resolve().parent
REPO = AGENT_DIR.parent
TASKS = AGENT_DIR / "tasks"
DONE = TASKS / "done"
LOGS = AGENT_DIR / "logs"
REPORTS = AGENT_DIR / "reports"

STANDING_ORDERS = """
You are working autonomously in the Cartload repository. Nobody will answer questions, so make
reasonable decisions yourself and record them in your report. Read CLAUDE.md and
docs/device-protocol.md before touching code. Finish the whole brief; if something blocks part of
it, finish the rest and say exactly what is left. Verify your work by building and running tests.
Commit when done. Write the report to .agent/reports/{name}.md.

The brief:
"""

ALLOWED_TOOLS = [
    "Read", "Write", "Edit", "MultiEdit", "Glob", "Grep", "Bash",
    "WebSearch", "WebFetch", "TodoWrite", "Task",
]


def ledger_total() -> float:
    """Sum of the estimated cost column in logs/costs.tsv (0 if absent)."""
    path = LOGS / "costs.tsv"
    if not path.exists():
        return 0.0
    total = 0.0
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) >= 7:
            try:
                total += float(parts[6])
            except ValueError:
                pass
    return total


def summarize_tool(block: ToolUseBlock) -> str:
    inp = block.input or {}
    if block.name == "Bash":
        return (inp.get("description") or inp.get("command") or "")[:120]
    for key in ("file_path", "path", "pattern", "query", "url", "prompt", "description"):
        if key in inp:
            return str(inp[key])[:120]
    return ""


async def run_task(task: Path, args: argparse.Namespace) -> bool:
    name = task.stem
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    log_path = LOGS / f"{name}-{stamp}.log"
    log = log_path.open("w", encoding="utf-8")

    def out(line: str, echo: bool = True) -> None:
        log.write(line + "\n")
        log.flush()
        if echo:
            print(line, flush=True)

    prompt = STANDING_ORDERS.format(name=name) + task.read_text(encoding="utf-8")
    options = ClaudeAgentOptions(
        model=args.model,
        effort=args.effort,
        cwd=str(REPO),
        permission_mode="bypassPermissions",
        allowed_tools=ALLOWED_TOOLS,
        setting_sources=["project"],
        system_prompt={"type": "preset", "preset": "claude_code"},
        max_budget_usd=args.budget,
        max_turns=args.max_turns,
        env={"CLAUDE_AGENT_SDK_CLIENT_APP": "cartload-driver"},
        # Screenshot reads come back as one JSON line; the SDK default (1 MB) crashed task 09.
        max_buffer_size=64 * 1024 * 1024,
    )
    out(f"=== {name}  model={args.model} effort={args.effort} budget=${args.budget}  {stamp}")
    result: ResultMessage | None = None
    try:
        async for message in query(prompt=prompt, options=options):
            if isinstance(message, AssistantMessage):
                for block in message.content:
                    if isinstance(block, TextBlock):
                        out(block.text)
                    elif isinstance(block, ToolUseBlock):
                        out(f"  [{block.name}] {summarize_tool(block)}")
            elif isinstance(message, UserMessage):
                content = message.content if isinstance(message.content, list) else []
                for block in content:
                    if isinstance(block, ToolResultBlock) and block.is_error:
                        text = block.content if isinstance(block.content, str) else str(block.content)
                        out(f"  [tool error] {text[:300]}")
                    elif isinstance(block, ToolResultBlock):
                        text = block.content if isinstance(block.content, str) else str(block.content)
                        out(f"  [result] {text[:2000]}", echo=False)
            elif isinstance(message, SystemMessage):
                out(f"  [system:{message.subtype}]", echo=False)
            elif isinstance(message, ResultMessage):
                result = message
    except CLINotFoundError as e:
        out(f"!!! Claude Code CLI not found: {e}")
        return False
    except ProcessError as e:
        out(f"!!! agent process error: {e}")
        return False
    except KeyboardInterrupt:
        out("!!! interrupted by the user; the task's uncommitted work stays in the tree")
        raise
    except Exception as e:  # noqa: BLE001 - anything else must not take the chain down silently
        out(f"!!! driver exception: {type(e).__name__}: {e}")
        out(traceback.format_exc(), echo=False)
        with (LOGS / "costs.tsv").open("a", encoding="utf-8") as f:
            f.write(f"{stamp}\t{name}\t{args.model}\tcrashed:{type(e).__name__}\t?\t?\t0\n")
        return False
    finally:
        log.close()

    if result is None:
        print("!!! no result message received")
        with (LOGS / "costs.tsv").open("a", encoding="utf-8") as f:
            f.write(f"{stamp}\t{name}\t{args.model}\tno-result\t?\t?\t0\n")
        return False

    cost = result.total_cost_usd or 0.0
    mins = result.duration_ms / 60000
    ok = not result.is_error and result.subtype == "success"
    print(f"=== {name}: {result.subtype}  turns={result.num_turns}  {mins:.1f} min  est ${cost:.2f}")
    if result.errors:
        print("    errors:", "; ".join(result.errors)[:500])
    with (LOGS / "costs.tsv").open("a", encoding="utf-8") as f:
        f.write(f"{stamp}\t{name}\t{args.model}\t{result.subtype}\t{result.num_turns}\t{mins:.1f}\t{cost:.4f}\n")
    if ok:
        DONE.mkdir(exist_ok=True)
        shutil.move(str(task), DONE / task.name)
    return ok


async def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("tasks", nargs="*", help="brief files; default: all pending in tasks/")
    parser.add_argument("--model", default="claude-fable-5-1")
    parser.add_argument("--effort", default=None, help="low|medium|high|max (model default if unset)")
    parser.add_argument("--budget", type=float, default=25.0, help="USD cap per task (estimate)")
    parser.add_argument("--max-turns", type=int, default=None)
    parser.add_argument("--keep-going", action="store_true", help="continue after a failed task")
    parser.add_argument("--max-total", type=float, default=160.0,
                        help="stop starting tasks once the ledger total plus the per-task budget would exceed this USD amount")
    args = parser.parse_args()

    if not os.environ.get("ANTHROPIC_API_KEY"):
        print("ANTHROPIC_API_KEY is not set. Refusing to run so nothing bills to a subscription login.")
        return 2

    tasks = [Path(t).resolve() for t in args.tasks] or sorted(p for p in TASKS.glob("*.md"))
    if not tasks:
        print("no pending tasks in", TASKS)
        return 0
    LOGS.mkdir(exist_ok=True)
    REPORTS.mkdir(exist_ok=True)
    print("tasks:", ", ".join(t.name for t in tasks))
    for task in tasks:
        spent = ledger_total()
        if spent + args.budget > args.max_total:
            print(f"ledger shows ${spent:.2f} spent; starting another task with a ${args.budget:.0f} budget "
                  f"would exceed --max-total ${args.max_total:.0f}. Stopping.")
            return 3
        ok = await run_task(task, args)
        if not ok and not args.keep_going:
            print("stopping after failure; rerun with --keep-going to continue past failures")
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
