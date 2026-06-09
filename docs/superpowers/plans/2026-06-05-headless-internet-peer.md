# Headless Internet Peer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a solo Friend Connect test mode with a visible P1 host and a headless internet P2 peer using the same Supabase setup, direct UDP gameplay packets, delay buffer, and rollback session as real Friend Connect.

**Architecture:** Add a headless runtime mode in `crates/mole_runtime/src/main.rs` and a bounded JSONL logger in `crates/mole_runtime/src/lib.rs`. Package scripts expose the mode through launchers without changing the existing real friend flow.

**Tech Stack:** Rust, Cargo workspace, SDL3/WUP visible host, Supabase setup signaling, direct UDP transport, custom rollback, JSONL diagnostic logs.

---

### Task 1: Runtime Argument and Config Contract

**Files:**
- Modify: `crates/mole_runtime/src/main.rs`

- [ ] **Step 1: Write failing tests**

Add tests that prove `--friend-connect-headless-peer --connect-code ABCD12`
parses into headless mode with code `ABCD12`, default delay `2`, and a default
frame budget suitable for unattended diagnostics.

- [ ] **Step 2: Run red test**

Run one filter:

```powershell
cargo test -p mole_runtime --features "sdl wup" --bin mole_runtime parse_headless_friend_peer -- --nocapture
```

Expected: fails because the parser/config does not exist yet.

- [ ] **Step 3: Implement minimal parser**

Create a small `HeadlessFriendPeerConfig` and parser helper in `main.rs`.

- [ ] **Step 4: Run green test**

Run the same one-filter command and confirm it passes.

### Task 2: Bounded Netplay Logger

**Files:**
- Modify: `crates/mole_runtime/src/lib.rs`
- Modify: `crates/mole_runtime/tests/runtime_contract.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving a bounded JSONL writer emits compact events, stops at the byte
cap, and records a single `log_cap_reached` event.

- [ ] **Step 2: Run red test**

```powershell
cargo test -p mole_runtime --test runtime_contract bounded_netplay_log -- --nocapture
```

Expected: fails because the logger does not exist yet.

- [ ] **Step 3: Implement minimal logger**

Add `BoundedNetplayLogger`, `NetplayLogRole`, `NetplayLogEvent`, and helpers.
Keep writes synchronous and tiny; do not log every packet.

- [ ] **Step 4: Run green test**

Run the same filter and confirm it passes.

### Task 3: Headless Peer Runtime Loop

**Files:**
- Modify: `crates/mole_runtime/src/main.rs`

- [ ] **Step 1: Write failing tests**

Add tests for deterministic headless neutral input and startup status strings.

- [ ] **Step 2: Run red test**

```powershell
cargo test -p mole_runtime --features "sdl wup" --bin mole_runtime headless_friend_peer -- --nocapture
```

Expected: fails because the helper path does not exist yet.

- [ ] **Step 3: Implement minimal headless loop**

Use Supabase `join_direct_endpoint`, direct UDP transport, match-start listener,
`InputDelayBuffer`, `RollbackSession`, and recent input retransmission. The peer
is P2, remote is P1, and local input is neutral until scripted input exists.

- [ ] **Step 4: Run green test**

Run the same filter and confirm it passes.

### Task 4: Package and CLI Surface

**Files:**
- Modify: `tools/package_friend_playtest.ps1`
- Modify: `crates/mole_cli/src/lib.rs`
- Modify: `crates/mole_cli/tests/cli_contract.rs`
- Modify: `README.md`
- Modify: `execs/README.md`

- [ ] **Step 1: Write failing CLI/package tests**

Extend `friend-connect status` to advertise solo internet testing and package
launchers.

- [ ] **Step 2: Run red test**

```powershell
cargo test -p mole_cli --test cli_contract friend_connect_status_reports_role_controller_and_package_contract -- --nocapture
```

Expected: fails until status output includes the solo-test contract.

- [ ] **Step 3: Implement package/status/docs**

Add a headless peer launcher and README text. Keep existing launchers unchanged
except where they mention the solo mode.

- [ ] **Step 4: Run green test**

Run the same CLI test and confirm it passes.

### Task 5: Verification and Rebuild

**Files:**
- Refresh: `playtest/MoleGame-FriendPlaytest.exe`

- [ ] **Step 1: Format and check**

```powershell
cargo fmt --check
cargo check -p mole_runtime --features "sdl wup"
```

- [ ] **Step 2: Focused tests**

```powershell
cargo test -p mole_runtime --features "sdl wup" --bin mole_runtime -- --nocapture
cargo test -p mole_runtime --test runtime_contract bounded_netplay_log -- --nocapture
cargo test -p mole_cli --test cli_contract friend_connect_status_reports_role_controller_and_package_contract -- --nocapture
cargo test -p mole_signaling --test signaling_contract -- --nocapture
cargo test -p mole_transport --test transport_contract -- --nocapture
```

- [ ] **Step 3: Rebuild package**

```powershell
cargo run -p mole_cli -- package friend-playtest --json
```

- [ ] **Step 4: Report artifact**

Hash `playtest/MoleGame-FriendPlaytest.exe` and report the path/hash to the
user.
