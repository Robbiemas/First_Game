use std::time::{Duration, Instant};

use mole_core::{Frame, PlayerInput};
use mole_rollback::RollbackSession;
use mole_runtime::{
    default_play_world, preload_runtime_source_frame_data, step_world_with_source_collisions,
};

const DEFAULT_SAMPLES: usize = 300;

fn main() -> Result<(), String> {
    let samples = std::env::args()
        .nth(1)
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|error| format!("invalid sample count: {error}"))?
        .unwrap_or(DEFAULT_SAMPLES)
        .max(10);
    preload_runtime_source_frame_data()?;

    let ordinary = benchmark_ordinary_simulation(samples);
    print_summary("ordinary", 0, &ordinary, 0, 0);

    for depth in 1..=8u32 {
        let (durations, retained_records, retained_inputs) = benchmark_rollback(samples, depth);
        print_summary(
            "rollback",
            depth,
            &durations,
            retained_records,
            retained_inputs,
        );
    }
    Ok(())
}

fn benchmark_ordinary_simulation(samples: usize) -> Vec<Duration> {
    let mut world = default_play_world();
    let mut durations = Vec::with_capacity(samples);
    for frame_number in 0..samples as u32 {
        let inputs = benchmark_inputs(Frame(frame_number));
        let start = Instant::now();
        step_world_with_source_collisions(&mut world, Frame(frame_number), &inputs);
        durations.push(start.elapsed());
    }
    std::hint::black_box(world.checksum());
    durations
}

fn benchmark_rollback(samples: usize, depth: u32) -> (Vec<Duration>, usize, usize) {
    let mut durations = Vec::with_capacity(samples);
    let mut retained_records = 0;
    let mut retained_inputs = 0;
    for sample in 0..samples as u32 {
        let mut session = RollbackSession::new_with_step(
            default_play_world(),
            16,
            step_world_with_source_collisions,
        );
        for frame_number in 0..depth {
            let inputs = benchmark_inputs(Frame(frame_number + sample));
            session.advance_with_prediction(Frame(frame_number), [Some(inputs[0]), None]);
        }
        let corrected = benchmark_inputs(Frame(sample))[1]
            .with_attack(true)
            .with_special(sample % 2 == 0);
        let start = Instant::now();
        assert!(session.confirm_input(Frame(0), 1, corrected, Frame(depth)));
        durations.push(start.elapsed());
        retained_records = session.retained_frame_record_count();
        retained_inputs = session.retained_input_frame_count();
        std::hint::black_box(session.world().checksum());
    }
    (durations, retained_records, retained_inputs)
}

fn benchmark_inputs(frame: Frame) -> [PlayerInput; 2] {
    [
        PlayerInput::neutral()
            .with_left_stick(if frame.0 % 4 < 2 { 96 } else { -96 }, 0)
            .with_attack(frame.0 % 7 == 3),
        PlayerInput::neutral()
            .with_left_stick(if frame.0 % 6 < 3 { -96 } else { 96 }, 0)
            .with_special(frame.0 % 11 == 5),
    ]
}

fn print_summary(
    kind: &str,
    depth: u32,
    durations: &[Duration],
    retained_records: usize,
    retained_inputs: usize,
) {
    let mut nanos = durations.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    nanos.sort_unstable();
    println!(
        "kind={kind} depth={depth} samples={} p50_ns={} p95_ns={} p99_ns={} max_ns={} retained_records={retained_records} retained_inputs={retained_inputs}",
        nanos.len(),
        percentile(&nanos, 50),
        percentile(&nanos, 95),
        percentile(&nanos, 99),
        nanos.last().copied().unwrap_or_default(),
    );
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    let index = (sorted.len().saturating_sub(1) * percentile) / 100;
    sorted[index]
}
