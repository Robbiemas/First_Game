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
cargo run -p mole_cli -- replay check --replay replays\Game_20260530T214929.slp --frames 1800 --json
cargo run -p mole_cli -- replay check --inputs debug\slippi\Game_20260530T214929.inputs.json --mode seeded --json
cargo run -p mole_cli -- replay scan --inputs debug\slippi\Game_20260530T214929.inputs.json --lookahead 5 --json
cargo run -p mole_cli -- replay trace --inputs debug\slippi\Game_20260530T214929.inputs.json --player 2 --start 900 --end 934 --frames 1800 --format markdown
cargo run -p mole_cli -- decomp search ftCo_Turn_Anim --json
cargo run -p mole_cli -- decomp symbol ftCo_LandingFallSpecial_Enter --format markdown
cargo run -p mole_cli -- decomp show src/melee/ft/chara/ftCommon/ftCo_Turn.c --line 90 --context 24 --json
cargo run -p mole_cli -- frame-data extract --character dolphin_mole --source-character captain --state AttackAirN --write --json
cargo run -p mole_cli -- frame-data show --character dolphin_mole --state AttackAirN --format markdown
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

Replay-parity agents should use `replay scan` before choosing the next parity
fix. It groups start-to-finish replay divergences into scenario runs, replays a
short rollback lookahead from the pre-divergence snapshot, and records whether
the scenario realigns. Use `replay trace` on the highest-priority scenario when
you need the bounded per-frame source window for decomp lookup.

Replay-parity agents should use the `decomp` commands before falling back to
manual repository searches:

- `decomp search <query>` returns compact matches across the local
  `..\.research\doldecomp-melee` checkout, including suggested follow-up
  `decomp show` commands.
- `decomp symbol <name>` is tuned for source function names and ranks exact
  definitions before references.
- `decomp show <relative-path> --line N --context N` returns a bounded,
  numbered excerpt so agents can cite the decomp without dumping whole files.

Use `frame-data extract` and `frame-data show` when an agent needs attack data
or per-action collision data such as Captain Falcon Nair. `frame-data extract`
can update or initialize `resources/melee/frame_data/<character>/<state>.json`
with `--write`, preserves source `{x, y, z}` coordinates, and records projection
metadata for the current 2D view.

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
