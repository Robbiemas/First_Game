# Mole CLI

Agent-facing project assistant for Mole Game development.

The CLI is optimized for automation:

- JSON output by default.
- No interactive prompts.
- Nonzero exit code for usage errors.
- Read-only commands by default.
- Explicit request-board mutations through `request add` and `request done`.
- Stable top-level `schema_version`.

Useful commands:

```powershell
cargo run -p mole_cli -- help --json
cargo run -p mole_cli -- status --json
cargo run -p mole_cli -- agent brief --json
cargo run -p mole_cli -- agent brief --format markdown
cargo run -p mole_cli -- parity --json
cargo run -p mole_cli -- parity snapshot --json
cargo run -p mole_cli -- snapshot --json
cargo run -p mole_cli -- graph missing --json
cargo run -p mole_cli -- graph next --json
cargo run -p mole_cli -- graph next --format markdown
cargo run -p mole_cli -- graph inspect Dash --json
cargo run -p mole_cli -- graph inspect "Dash -> Run" --format markdown
cargo run -p mole_cli -- verify changed --json
cargo run -p mole_cli -- verify changed --format markdown
cargo run -p mole_cli -- generated check --json
cargo run -p mole_cli -- generated check --format markdown
cargo run -p mole_cli -- finish check --json
cargo run -p mole_cli -- finish check --format markdown
cargo run -p mole_cli -- doctor --json
cargo run -p mole_cli -- tests --json
cargo run -p mole_cli -- handoff --json
cargo run -p mole_cli -- handoff --format markdown
cargo run -p mole_cli -- recommend-next --json
cargo run -p mole_cli -- request list --json
cargo run -p mole_cli -- request next --json
```

Use `cargo run -p mole_cli -- help --json` as the canonical agent-facing
feature index. The help payload lists every command, purpose, mutation behavior,
written files, flags, aliases, output modes, and examples. Update it whenever a
Mole CLI command is added, removed, or materially changed.

Use `--root "D:\Mole Game\First_Game"` when calling from outside the repository.

After an agent's current primary task is verified complete, it may inspect
`MOLE_CLI_AGENT_MESSAGES.md` during a lapse in its directive. If the newest
Inbox message requests a CLI or development-tooling feature, that agent may take
up the request as temporary maintenance work with tests unless a newer direct
user instruction overrides it.

Feature request workflow:

```powershell
cargo run -p mole_cli -- request add --id trace-summary --title "Trace Summary" --request "Add a compact trace summary command." --context "Agents need shorter logs." --expected "JSON summary." --json
cargo run -p mole_cli -- request next --json
cargo run -p mole_cli -- request done --id trace-summary --result "Implemented request workflow." --json
```

`request add` inserts a newest-first Inbox item into
`MOLE_CLI_AGENT_MESSAGES.md`. `request next` reads the newest Inbox item without
mutating the workspace. `request done` moves the matching Inbox item into
Completed Notes and records the result.
