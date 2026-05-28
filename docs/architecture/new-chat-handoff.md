# New Chat Handoff Message

Copy and send this message at the start of the new chat.

```text
We are continuing work on Robbiemas/First_Game in D:\Mole Game\First_Game.

GitHub repository:

- Remote: https://github.com/Robbiemas/First_Game
- Work branch: handoff/rust-rollback-architecture
- Use GitHub CLI / normal git CLI only. Do not use the GitHub plugin or GitHub connector tools in Codex; they are not currently functional for this project.

The goal is to move the project from the legacy Pygame prototype into a native Rust deterministic platform-fighter architecture optimized for low latency, GameCube-native input, Melee/UCF-like movement feel, and eventual peer-to-peer rollback netplay.

Please start by reading these docs:

1. docs/architecture/native-rust-rollback-architecture.md
2. docs/superpowers/plans/2026-05-28-native-rust-rollback-migration.md
3. docs/research/melee-input-state-reference.md
4. docs/research/mole-state-coverage-comparison.md
5. docs/research/pygame-movement-logic-pass.md
6. docs/superpowers/specs/2026-05-27-rust-rollback-core-design.md

Important context:

- The future architecture is Rust authoritative, not Pygame authoritative.
- Pygame is now a temporary harness/reference, not the long-term movement engine.
- The Rust core should own deterministic 60 Hz simulation, motion states, physics, checksums, replay, and rollback.
- SDL3 should become the native runtime shell for window, audio, generic input fallback, and rendering.
- Native WUP-028/GameCube controller input should be first-class.
- GameCube input should preserve raw main stick, C-stick, L/R analog trigger bytes, L/R digital trigger bottom-out, D-pad, and buttons.
- UCF should be implemented natively in the input layer, as a controller preprocessing/default behavior, not as a Pygame mechanics patch.
- Supabase, if used, should be for lobby/signaling/matchmaking only. It should not carry 60 Hz gameplay input.
- Direct UDP should be the first real network gameplay transport. WebRTC DataChannel can be added later behind the same transport interface.
- There should be no authoritative gameplay server. Each peer owns its local inputs; deterministic simulation plus rollback owns world state.

Please do not continue the old Pygame hotfix spiral unless it is only to keep a temporary launcher working. The clean launch point is:

1. Verify workspace status and current tests.
2. Begin Phase 1 and Phase 2 from docs/superpowers/plans/2026-05-28-native-rust-rollback-migration.md.
3. First implementation target: make Rust core own Wait -> WalkSlow/WalkMiddle/WalkFast and the related input facts, then lock it with tests.

Before editing, summarize what you read from the docs and tell me the first concrete implementation checkpoint you are going to execute.
```
