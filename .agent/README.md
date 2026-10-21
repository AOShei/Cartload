# Agent driver

Runs the briefs in `tasks/` with the Claude Agent SDK, billed to the API key in
`ANTHROPIC_API_KEY` (never to a claude.ai subscription; the driver refuses to start without the key).

```
cd .agent
export ANTHROPIC_API_KEY=sk-ant-...
uv run run.py                 # all pending briefs, in order
uv run run.py tasks/02-fel.md # one brief
```

Options: `--model` (default `claude-fable-5-1`), `--effort`, `--budget` (USD per task, default 25),
`--max-turns`, `--keep-going`. Logs in `logs/`, cost ledger in `logs/costs.tsv`, agent reports in
`reports/`. Finished briefs move to `tasks/done/`.

The agent runs with `bypassPermissions` inside this repository. Keep anything you care about
out of this tree, and review `git log` after each run.
