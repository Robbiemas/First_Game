#!/usr/bin/env node

"use strict";

const assert = require("assert");
const fs = require("fs");
const path = require("path");

const PROJECT_ROOT = path.resolve(__dirname, "..");
const DEPENDENCY_INSTALL_COMMAND = "npm install --prefix tools/slippi";
const SLIPPI_NODE_ENTRYPOINT = "@slippi/slippi-js/node";
const DEFAULT_EXPORT_FRAMES = 1800;
const MELEE_MAIN_STICK_DEADZONE_X = 36;
const MELEE_TAP_X_THRESHOLD = 32;
const MELEE_DASH_X = 102;
const MAX_MELEE_INPUT_TIMER = 0xfe;
const UCF_PAD_BUFFER_SIZE = 4;
const UCF_PAD_BUFFER_MASK = UCF_PAD_BUFFER_SIZE - 1;
const UCF_TILT_INTENT_DELTA = 75;

const HSD_BUTTON_BITS = [
  [0, "dpad_left"],
  [1, "dpad_right"],
  [2, "dpad_down"],
  [3, "dpad_up"],
  [4, "z"],
  [5, "r"],
  [6, "l"],
  [8, "a"],
  [9, "b"],
  [10, "x"],
  [11, "y"],
  [12, "start"],
  [31, "lr"],
];

const IMPORTANT_STATE_NAMES = new Map([
  [14, "Wait"],
  [15, "WalkSlow"],
  [16, "WalkMiddle"],
  [17, "WalkFast"],
  [18, "Turn"],
  [19, "TurnRun"],
  [20, "Dash"],
  [21, "Run"],
  [22, "RunDirect"],
  [23, "RunBrake"],
  [24, "KneeBend"],
  [25, "JumpF"],
  [26, "JumpB"],
  [27, "JumpAerialF"],
  [28, "JumpAerialB"],
  [29, "Fall"],
  [30, "FallF"],
  [31, "FallB"],
  [32, "FallAerial"],
  [33, "FallAerialF"],
  [34, "FallAerialB"],
  [42, "Landing"],
  [43, "LandingFallSpecial"],
  [178, "GuardOn"],
  [179, "Guard"],
  [180, "GuardOff"],
  [181, "GuardSetOff"],
  [182, "GuardReflect"],
  [233, "EscapeF"],
  [234, "EscapeB"],
  [235, "EscapeN"],
  [236, "EscapeAir"],
  [244, "Pass"],
  [252, "CliffCatch"],
  [253, "CliffWait"],
  [322, "Entry"],
  [323, "EntryStart"],
  [324, "EntryEnd"],
  [349, "SpecialSStart"],
  [350, "SpecialS"],
  [351, "SpecialAirSStart"],
  [352, "SpecialAirS"],
]);

function usage() {
  return [
    "Usage:",
    "  node tools/slippi_replay_to_inputs.cjs --replay replays/Game.slp [--frames 1800]",
    "  node tools/slippi_replay_to_inputs.cjs --self-test",
    "",
    "Options:",
    "  --replay <path>   Slippi .slp file to export.",
    "  --out <path>      JSON output path. Defaults to debug/slippi/<name>.inputs.json.",
    "  --report <path>   Markdown report path. Defaults to debug/slippi/<name>.report.md.",
    "  --frames <count>  Export this many non-negative frames. Defaults to 1800.",
    "  --include-negative-frames  Include pre-game frames such as Entry/EntryStart/EntryEnd.",
    "  --all-frames      Export the full replay.",
    "  --self-test       Run pure conversion checks without loading slippi-js.",
    "",
    `If parsing fails because the Slippi parser is missing, run: ${DEPENDENCY_INSTALL_COMMAND}`,
  ].join("\n");
}

function parseArgs(argv) {
  const options = {
    replayPath: null,
    outPath: null,
    reportPath: null,
    frames: DEFAULT_EXPORT_FRAMES,
    includeNegativeFrames: false,
    selfTest: false,
    help: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--self-test") {
      options.selfTest = true;
    } else if (arg === "--help" || arg === "-h") {
      options.help = true;
    } else if (arg === "--all-frames") {
      options.frames = null;
    } else if (arg === "--include-negative-frames") {
      options.includeNegativeFrames = true;
    } else if (arg === "--replay") {
      options.replayPath = requireValue(argv, ++i, arg);
    } else if (arg === "--out") {
      options.outPath = requireValue(argv, ++i, arg);
    } else if (arg === "--report") {
      options.reportPath = requireValue(argv, ++i, arg);
    } else if (arg === "--frames") {
      options.frames = parsePositiveInteger(requireValue(argv, ++i, arg), arg);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  return options;
}

function requireValue(argv, index, flag) {
  const value = argv[index];
  if (!value || value.startsWith("--")) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function parsePositiveInteger(value, flag) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive integer`);
  }
  return parsed;
}

function loadSlippiNode() {
  const candidates = [
    SLIPPI_NODE_ENTRYPOINT,
    path.join(__dirname, "slippi", "node_modules", "@slippi", "slippi-js", "node"),
  ];

  for (const candidate of candidates) {
    try {
      return require(candidate);
    } catch (error) {
      if (error.code !== "MODULE_NOT_FOUND") {
        throw error;
      }
    }
  }

  throw new Error(
    [
      `Missing ${SLIPPI_NODE_ENTRYPOINT}.`,
      `Run: ${DEPENDENCY_INSTALL_COMMAND}`,
      "This dependency is intentionally local to tools/slippi so gameplay crates stay Rust-only.",
    ].join("\n"),
  );
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function slippiStickToNative(value) {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return clamp(Math.round(value * 127), -127, 127);
}

function slippiTriggerToByte(value) {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return clamp(Math.round(value * 255), 0, 255);
}

function buttonNames(bits) {
  const numericBits = Number.isFinite(bits) ? bits : 0;
  return HSD_BUTTON_BITS.filter(([bit]) => (numericBits & (1 << bit)) !== 0).map(
    ([, name]) => name,
  );
}

function buildStateNameLookup(State) {
  const lookup = new Map(IMPORTANT_STATE_NAMES);
  for (const [name, value] of Object.entries(State || {})) {
    if (Number.isInteger(value) && !lookup.has(value)) {
      lookup.set(value, name);
    }
  }
  return lookup;
}

function stateName(actionStateId, lookup) {
  return lookup.get(actionStateId) || `ActionState${actionStateId}`;
}

function defaultOutputPath(replayPath, suffix) {
  const name = path.basename(replayPath, path.extname(replayPath));
  return path.join(PROJECT_ROOT, "debug", "slippi", `${name}${suffix}`);
}

function playerSettingsByIndex(settings) {
  const players = {};
  for (const player of settings.players || []) {
    players[player.playerIndex] = {
      player_index: player.playerIndex,
      port: player.port,
      character_id: player.characterId,
      display_name: player.displayName || "",
      connect_code: player.connectCode || "",
      controller_fix: player.controllerFix || "",
      type: player.type,
    };
  }
  return players;
}

function convertPreFrame(pre) {
  if (!pre) {
    return null;
  }

  return {
    action_state_id: pre.actionStateId,
    position: roundedPair(pre.positionX, pre.positionY),
    facing: pre.facingDirection,
    main_stick: roundedPair(pre.joystickX, pre.joystickY),
    c_stick: roundedPair(pre.cStickX, pre.cStickY),
    trigger: roundFloat(pre.trigger),
    physical_l_trigger: roundFloat(pre.physicalLTrigger),
    physical_r_trigger: roundFloat(pre.physicalRTrigger),
    raw_joystick_x: numberOrNull(pre.rawJoystickX),
    raw_joystick_y: numberOrNull(pre.rawJoystickY),
    rust_player_input: {
      stick_x: slippiStickToNative(pre.joystickX),
      stick_y: slippiStickToNative(pre.joystickY),
      c_stick_x: slippiStickToNative(pre.cStickX),
      c_stick_y: slippiStickToNative(pre.cStickY),
      left_trigger: slippiTriggerToByte(pre.physicalLTrigger ?? pre.trigger),
      right_trigger: slippiTriggerToByte(pre.physicalRTrigger ?? pre.trigger),
      physical_button_bits: pre.physicalButtons || 0,
      physical_buttons: buttonNames(pre.physicalButtons),
      processed_button_bits: pre.buttons || 0,
      processed_buttons: buttonNames(pre.buttons),
    },
  };
}

function convertPostFrame(post, stateLookup) {
  if (!post) {
    return null;
  }

  return {
    action_state_id: post.actionStateId,
    action_state_name: stateName(post.actionStateId, stateLookup),
    action_state_counter: roundFloat(post.actionStateCounter),
    position: roundedPair(post.positionX, post.positionY),
    facing: post.facingDirection,
    airborne: Boolean(post.isAirborne),
    last_ground_id: post.lastGroundId,
    jumps_remaining: post.jumpsRemaining,
    shield_size: roundFloat(post.shieldSize),
    self_induced_speeds: {
      air_x: roundFloat(post.selfInducedSpeeds?.airX),
      y: roundFloat(post.selfInducedSpeeds?.y),
      attack_x: roundFloat(post.selfInducedSpeeds?.attackX),
      attack_y: roundFloat(post.selfInducedSpeeds?.attackY),
      ground_x: roundFloat(post.selfInducedSpeeds?.groundX),
    },
  };
}

function roundedPair(x, y) {
  return [roundFloat(x), roundFloat(y)];
}

function roundFloat(value) {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.round(value * 1_000_000) / 1_000_000;
}

function numberOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function exportReplay(replayPath, frameLimit, includeNegativeFrames = false) {
  const { SlippiGame, State } = loadSlippiNode();
  const game = new SlippiGame(replayPath);
  const settings = game.getSettings();
  const metadata = game.getMetadata();
  const frames = game.getFrames();
  const stateLookup = buildStateNameLookup(State);
  const playerSettings = playerSettingsByIndex(settings);
  const frameNumbers = Object.keys(frames)
    .map(Number)
    .filter((frame) => includeNegativeFrames || frame >= 0)
    .sort((a, b) => a - b);
  const selectedFrames =
    frameLimit === null ? frameNumbers : frameNumbers.slice(0, frameLimit);

  const exportedFrames = selectedFrames.map((frameNumber) => {
    const frame = frames[frameNumber];
    const players = {};
    for (const [playerIndex, framePlayer] of Object.entries(frame.players || {})) {
      players[playerIndex] = {
        pre: convertPreFrame(framePlayer.pre),
        post: convertPostFrame(framePlayer.post, stateLookup),
      };
    }

    return {
      frame: frameNumber,
      players,
    };
  });
  annotateUcfDashbackAmendments(exportedFrames, playerSettings);

  return {
    schema_version: 1,
    source: {
      replay_path: path.resolve(replayPath),
      parser: SLIPPI_NODE_ENTRYPOINT,
      parser_note:
        "Slippi pre-frame joystick floats are exported as observed; rust_player_input scales them with round(value * 127) into HSD-clamped signed bytes before the Rust core derives fighter f32 stick values. UCF replay metadata derives adapter-owned dashback amendment bits from raw stick history without rewriting the already recorded game-facing stick value.",
    },
    settings: {
      slp_version: settings.slpVersion,
      stage_id: settings.stageId,
      is_teams: settings.isTeams,
      is_pal: settings.isPAL,
      timer_type: settings.timerType,
      starting_timer_seconds: settings.startingTimerSeconds,
      players: playerSettings,
    },
    metadata: {
      start_at: metadata.startAt,
      last_frame: metadata.lastFrame,
      played_on: metadata.playedOn,
    },
    export: {
      first_frame: selectedFrames[0] ?? null,
      last_frame: selectedFrames[selectedFrames.length - 1] ?? null,
      frame_count: selectedFrames.length,
      requested_frame_limit: frameLimit,
      included_negative_frames: includeNegativeFrames,
    },
    frames: exportedFrames,
    diagnostics: buildDiagnostics(exportedFrames),
  };
}

function buildDiagnostics(frames) {
  const players = {};

  for (const frame of frames) {
    for (const [playerIndex, playerFrame] of Object.entries(frame.players)) {
      if (!players[playerIndex]) {
        players[playerIndex] = {
          state_transitions: [],
          moonwalk_like_dash_frames: [],
          max_abs_ground_speed: 0,
          max_abs_air_speed: 0,
        };
      }

      const stats = players[playerIndex];
      const post = playerFrame.post;
      if (!post) {
        continue;
      }

      const previous =
        stats.state_transitions.length > 0
          ? stats.state_transitions[stats.state_transitions.length - 1]
          : null;
      if (!previous || previous.to_state_id !== post.action_state_id) {
        stats.state_transitions.push({
          frame: frame.frame,
          to_state_id: post.action_state_id,
          to_state_name: post.action_state_name,
          facing: post.facing,
          ground_x: post.self_induced_speeds.ground_x,
          air_x: post.self_induced_speeds.air_x,
          position_x: post.position[0],
        });
      }

      const groundX = post.self_induced_speeds.ground_x;
      const airX = post.self_induced_speeds.air_x;
      stats.max_abs_ground_speed = Math.max(stats.max_abs_ground_speed, Math.abs(groundX));
      stats.max_abs_air_speed = Math.max(stats.max_abs_air_speed, Math.abs(airX));

      if (
        post.action_state_id === 20 &&
        post.facing !== 0 &&
        Number.isFinite(groundX) &&
        groundX * post.facing < -0.1 &&
        stats.moonwalk_like_dash_frames.length < 40
      ) {
        stats.moonwalk_like_dash_frames.push({
          frame: frame.frame,
          facing: post.facing,
          ground_x: groundX,
          position_x: post.position[0],
          stick_x: playerFrame.pre?.main_stick?.[0] ?? 0,
          native_stick_x: playerFrame.pre?.rust_player_input?.stick_x ?? 0,
        });
      }
    }
  }

  for (const stats of Object.values(players)) {
    stats.max_abs_ground_speed = roundFloat(stats.max_abs_ground_speed);
    stats.max_abs_air_speed = roundFloat(stats.max_abs_air_speed);
    stats.state_transitions = stats.state_transitions.slice(0, 120);
  }

  return { players };
}

function annotateUcfDashbackAmendments(frames, playerSettings) {
  const states = {};

  for (const frame of frames) {
    for (const [playerIndex, playerFrame] of Object.entries(frame.players || {})) {
      const rustInput = playerFrame.pre?.rust_player_input;
      if (!rustInput) {
        continue;
      }

      if (!states[playerIndex]) {
        states[playerIndex] = newUcfDashbackState();
      }
      const state = states[playerIndex];
      const rawX = rawStickXFromPreFrame(playerFrame.pre);
      let processedX = nativeStickXFromPreFrame(playerFrame.pre);
      state.padBufferIndex = (state.padBufferIndex + 1) & UCF_PAD_BUFFER_MASK;
      state.padBuffer[state.padBufferIndex] = rawX;
      const previousRawX =
        state.padBuffer[
          (state.padBufferIndex + UCF_PAD_BUFFER_SIZE - 2) & UCF_PAD_BUFFER_MASK
        ];
      const cleanedX = cleanAxis(processedX, MELEE_MAIN_STICK_DEADZONE_X);
      state.stickXHoldTimer = updateAxisHoldTimer(
        state.stickXHoldTimer,
        state.previousCleanedX,
        cleanedX,
        MELEE_TAP_X_THRESHOLD,
      );
      state.previousCleanedX = cleanedX;

      rustInput.ucf_dashback_amendment =
        isUcfPlayer(playerSettings[playerIndex]) &&
        state.stickXHoldTimer < 2 &&
        Math.abs(processedX) >= MELEE_DASH_X &&
        ucfAxisDeltaExceeds(previousRawX, rawX, UCF_TILT_INTENT_DELTA);
    }
  }
}

function newUcfDashbackState() {
  return {
    padBuffer: [0, 0, 0, 0],
    padBufferIndex: 0,
    previousCleanedX: 0,
    stickXHoldTimer: MAX_MELEE_INPUT_TIMER,
  };
}

function isUcfPlayer(playerSetting) {
  return String(playerSetting?.controller_fix || "").toUpperCase() === "UCF";
}

function rawStickXFromPreFrame(pre) {
  if (Number.isFinite(pre?.raw_joystick_x)) {
    return clamp(Math.round(pre.raw_joystick_x), -127, 127);
  }
  return nativeStickXFromPreFrame(pre);
}

function nativeStickXFromPreFrame(pre) {
  return clamp(Math.round(pre?.rust_player_input?.stick_x || 0), -127, 127);
}

function cleanAxis(value, deadzone) {
  return Math.abs(value) < deadzone ? 0 : value;
}

function updateAxisHoldTimer(timer, previous, current, threshold) {
  if (current >= threshold) {
    return previous >= threshold ? incrementMeleeTimer(timer) : 0;
  }
  if (current <= -threshold) {
    return previous <= -threshold ? incrementMeleeTimer(timer) : 0;
  }
  return MAX_MELEE_INPUT_TIMER;
}

function incrementMeleeTimer(timer) {
  return Math.min(timer + 1, MAX_MELEE_INPUT_TIMER);
}

function ucfAxisDeltaExceeds(previous, current, threshold) {
  const delta = current - previous;
  return delta * delta > threshold * threshold;
}

function renderReport(exported) {
  const lines = [];
  lines.push("# Slippi Replay Input Diagnostic");
  lines.push("");
  lines.push(`- Replay: \`${exported.source.replay_path}\``);
  lines.push(`- Started: ${exported.metadata.start_at || "unknown"}`);
  lines.push(`- Stage id: ${exported.settings.stage_id}`);
  lines.push(`- Frames exported: ${exported.export.frame_count}`);
  lines.push(`- Frame range: ${exported.export.first_frame}..${exported.export.last_frame}`);
  lines.push("");
  lines.push(
    "This report preserves Slippi pre-frame analog values and post-frame Melee state/velocity facts. For UCF-tagged players it also exports adapter-owned UCF amendment bits, such as dashback, so replay diagnostics match the input layer without baking UCF into core mechanics.",
  );
  lines.push("");

  for (const [playerIndex, player] of Object.entries(exported.settings.players)) {
    const diagnostics = exported.diagnostics.players[playerIndex];
    lines.push(`## Player ${Number(playerIndex) + 1}`);
    lines.push("");
    lines.push(`- Port: ${player.port}`);
    lines.push(`- Character id: ${player.character_id}`);
    lines.push(`- Controller fix: ${player.controller_fix || "unknown"}`);
    lines.push(`- Max abs ground speed: ${diagnostics?.max_abs_ground_speed ?? 0}`);
    lines.push(`- Max abs air speed: ${diagnostics?.max_abs_air_speed ?? 0}`);
    lines.push("");

    lines.push("### First State Transitions");
    lines.push("");
    lines.push("| Frame | State | Facing | Ground X | Air X | Position X |");
    lines.push("| ---: | --- | ---: | ---: | ---: | ---: |");
    for (const transition of (diagnostics?.state_transitions || []).slice(0, 30)) {
      lines.push(
        `| ${transition.frame} | ${transition.to_state_name} (${transition.to_state_id}) | ${transition.facing} | ${transition.ground_x} | ${transition.air_x} | ${transition.position_x} |`,
      );
    }
    lines.push("");

    lines.push("### Moonwalk-Like Dash Evidence");
    lines.push("");
    const moonwalkFrames = diagnostics?.moonwalk_like_dash_frames || [];
    if (moonwalkFrames.length === 0) {
      lines.push("No dash frames in this export had ground velocity opposite facing beyond 0.1 units/frame.");
    } else {
      lines.push("| Frame | Facing | Ground X | Position X | Stick X | Native Stick X |");
      lines.push("| ---: | ---: | ---: | ---: | ---: | ---: |");
      for (const row of moonwalkFrames.slice(0, 20)) {
        lines.push(
          `| ${row.frame} | ${row.facing} | ${row.ground_x} | ${row.position_x} | ${row.stick_x} | ${row.native_stick_x} |`,
        );
      }
    }
    lines.push("");
  }

  return `${lines.join("\n").trimEnd()}\n`;
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function writeText(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, value, "utf8");
}

function runSelfTest() {
  assert.strictEqual(slippiStickToNative(1.0), 127);
  assert.strictEqual(slippiStickToNative(-1.0), -127);
  assert.strictEqual(slippiStickToNative(0.7875), 100);
  assert.strictEqual(slippiStickToNative(-0.7875), -100);
  assert.strictEqual(slippiStickToNative(0), 0);
  assert.strictEqual(slippiTriggerToByte(1.0), 255);
  assert.deepStrictEqual(buttonNames((1 << 8) | (1 << 10)), ["a", "x"]);

  const dashbackFrames = [
    slippiUcfSelfTestFrame(46, 1, 0),
    slippiUcfSelfTestFrame(47, -28, -44),
    slippiUcfSelfTestFrame(48, -90, -119),
  ];
  annotateUcfDashbackAmendments(dashbackFrames, {
    0: { controller_fix: "UCF" },
  });
  assert.strictEqual(
    dashbackFrames[0].players["0"].pre.rust_player_input.ucf_dashback_amendment,
    false,
  );
  assert.strictEqual(
    dashbackFrames[1].players["0"].pre.rust_player_input.ucf_dashback_amendment,
    false,
  );
  assert.strictEqual(
    dashbackFrames[2].players["0"].pre.rust_player_input.ucf_dashback_amendment,
    true,
  );
  assert.strictEqual(dashbackFrames[2].players["0"].pre.rust_player_input.stick_x, -119);
  assert.strictEqual(dashbackFrames[2].players["0"].pre.rust_player_input.stick_y, 0);

  const vanillaFrames = [
    slippiUcfSelfTestFrame(46, 1, 0),
    slippiUcfSelfTestFrame(47, -28, -44),
    slippiUcfSelfTestFrame(48, -90, -119),
  ];
  annotateUcfDashbackAmendments(vanillaFrames, {
    0: { controller_fix: "" },
  });
  assert.strictEqual(
    vanillaFrames[2].players["0"].pre.rust_player_input.ucf_dashback_amendment,
    false,
  );

  const gameFacingFrames = [
    slippiUcfSelfTestFrame(0, 0, 0),
    slippiUcfSelfTestFrame(1, -99, slippiStickToNative(-0.9875)),
  ];
  annotateUcfDashbackAmendments(gameFacingFrames, {
    0: { controller_fix: "UCF" },
  });
  assert.strictEqual(
    gameFacingFrames[1].players["0"].pre.rust_player_input.ucf_dashback_amendment,
    true,
  );
  assert.strictEqual(
    gameFacingFrames[1].players["0"].pre.rust_player_input.stick_x,
    -125,
  );
  assert.strictEqual(
    gameFacingFrames[1].players["0"].pre.rust_player_input.stick_y,
    0,
  );
  console.log("slippi_replay_to_inputs self-test passed");
}

function slippiUcfSelfTestFrame(frame, rawJoystickX, stickX) {
  return {
    frame,
    players: {
      0: {
        pre: {
          raw_joystick_x: rawJoystickX,
          raw_joystick_y: 0,
          rust_player_input: {
            stick_x: stickX,
            stick_y: 0,
          },
        },
        post: null,
      },
    },
  };
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    console.log(usage());
    return;
  }
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  if (!options.replayPath) {
    throw new Error(`missing --replay\n\n${usage()}`);
  }

  const replayPath = path.resolve(PROJECT_ROOT, options.replayPath);
  const outPath = options.outPath
    ? path.resolve(PROJECT_ROOT, options.outPath)
    : defaultOutputPath(replayPath, ".inputs.json");
  const reportPath = options.reportPath
    ? path.resolve(PROJECT_ROOT, options.reportPath)
    : defaultOutputPath(replayPath, ".report.md");
  const exported = exportReplay(replayPath, options.frames, options.includeNegativeFrames);
  writeJson(outPath, exported);
  writeText(reportPath, renderReport(exported));
  console.log(`wrote_json=${outPath}`);
  console.log(`wrote_report=${reportPath}`);
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}

module.exports = {
  annotateUcfDashbackAmendments,
  buttonNames,
  exportReplay,
  renderReport,
  slippiStickToNative,
  slippiTriggerToByte,
};
