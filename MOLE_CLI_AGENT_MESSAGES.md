# Mole CLI Agent Messages

This file is the message board for Rust `mole_cli` development-helper requests.

Use this file when an agent or user wants to leave instructions, observations,
or requests for Mole CLI/development-tooling work without interrupting the main
gameplay/parity workflow.

## How To Use

- Keep messages short and actionable.
- Prefer appending new messages under `Inbox`.
- Include enough context for any agent to act without reading the full parent
  thread.
- Do not put urgent gameplay-engine directives here; this board is for CLI and
  development-assistant tooling.
- Agents should treat this file as input, not as proof that a task is complete.

## Operating Rule

After an agent's current primary task is fully verified complete, that agent may
use a lapse in its directive to read this file, inspect the newest actionable
`Inbox` message, and take up a Mole CLI/development-tooling request as temporary
maintenance work unless a newer direct user instruction overrides it. Keep the
work scoped to Mole CLI/development-assistant tooling, and keep default CLI
behavior read-only, non-interactive, and JSON-first.

## Inbox

Add new messages here, newest at the top.

```markdown
### YYYY-MM-DD - Short Title

Request:

Context:

Expected output:
```

## Active Notes

- Mole CLI is intended for AI agents, not direct human use.
- Default behavior should remain read-only, non-interactive, JSON-first, and
  stable enough for automation.
- Commands should reduce context load for the main agent by summarizing repo
  status, parity state, generated-artifact health, and recommended verification.
- Any agent may add low-risk read-only Mole CLI features after its primary task
  is verified complete when those features clearly optimize AI-agent context,
  verification, or handoff workflow.

## Completed Notes

Move or summarize completed message threads here if useful.

### finish-check-help-catalog - Finish check help catalog mismatch

Request:
Please align the Mole CLI help catalog with the new finish check command. Current workspace gate fails in crates/mole_cli/tests/cli_contract.rs because help output does not document finish check, and finish check reports help_catalog.ok=false.

Context:
Main movement-parity agent is blocked from a clean cargo test --workspace gate while the CLI side work is in flight. Fresh cargo test --workspace currently fails only in mole_cli tests: help_command_exposes_full_agent_command_catalog and finish_check_returns_read_only_completion_gate_packet.

Expected output:
cargo test -p mole_cli --test cli_contract passes, help --json includes finish check, and finish check --json reports help_catalog.ok=true.

Result:
Implemented finish check, updated help/README, verified cargo test -p mole_cli, help --json includes finish check, and finish check --json reports help_catalog.ok=true.


### missing-graph-entries - Missing Graph Entries Command

Request:
Add a read-only CLI command that lists graph nodes and edges whose status is missing, including id/from/to, notes, source refs, rust refs, and a compact recommended next field.

Context:
Main gameplay agents repeatedly need to inspect missing parity graph entries without ad-hoc JSON parsing while staying on the current implementation line.

Expected output:
mole graph missing --json returns stable schema_version JSON and mutates nothing.

Result:
Implemented read-only graph missing command with stable JSON output for missing graph nodes and edges, updated help/README documentation, and softened message-board wording so any agent can take up CLI maintenance after its primary task is verified complete. Verified with cargo test -p mole_cli.


### mole-cli-help-catalog - Mole CLI Help Catalog

Request:
Create and maintain a help command that exposes all available Mole CLI features for agents.

Context:
Agents should be able to discover command usage, mutating behavior, flags, aliases, and examples directly from the CLI.

Expected output:
JSON-first help catalog covering every Mole CLI command.

Result:
Expanded help into a full agent-facing command catalog, updated README guidance, added contract coverage, and verified the Mole CLI suite.


### mole-cli-request-workflow - Mole CLI Request Workflow

Request:
Create CLI commands to add, receive, and complete message-board feature requests.

Context:
Agent coordination should happen through MOLE_CLI_AGENT_MESSAGES.md without manual markdown editing.

Expected output:
JSON-first request add, request next, request list, and request done commands.

Result:
Implemented request add/list/next/done, documented the workflow, and verified the Mole CLI test suite.


### 2026-05-31 - Read-Only Parity Snapshot

Request:
Add or refine a read-only `mole_cli` command that prints a compact JSON parity
snapshot for the main agent.

Result:
Implemented `cargo run -p mole_cli -- parity snapshot --json` and the alias
`cargo run -p mole_cli -- snapshot --json`. The command reports branch/remote,
dirty generated-artifact status, value diff counts, Falcon ECB mapping counts,
unmapped derived states, graph status counts, recommendations, and verification
commands without mutating the workspace.
