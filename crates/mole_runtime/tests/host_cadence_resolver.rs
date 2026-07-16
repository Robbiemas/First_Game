use std::time::Duration;

use mole_runtime::{HostCadence, HostCadenceResolver};

fn sample(resolver: &mut HostCadenceResolver, micros: u64, deadline_missed: bool) {
    resolver.observe_host_pass(Duration::from_micros(micros), deadline_missed);
}

#[test]
fn promotes_only_after_a_long_sustained_floor() {
    let mut resolver = HostCadenceResolver::new(HostCadence::Hz60);

    for _ in 0..HostCadenceResolver::PROMOTION_SAMPLES - 1 {
        sample(&mut resolver, 2_000, false);
        assert_eq!(resolver.cadence(), HostCadence::Hz60);
    }

    sample(&mut resolver, 2_000, false);
    assert_eq!(resolver.cadence(), HostCadence::Hz120);

    for _ in 0..HostCadenceResolver::PROMOTION_SAMPLES {
        sample(&mut resolver, 2_000, false);
    }
    assert_eq!(resolver.cadence(), HostCadence::Hz180);
}

#[test]
fn deadline_misses_and_insufficient_work_floor_demote_quickly() {
    let mut resolver = HostCadenceResolver::new(HostCadence::Hz240);

    sample(&mut resolver, 2_000, true);
    assert_eq!(resolver.cadence(), HostCadence::Hz180);

    sample(&mut resolver, 8_000, false);
    assert_eq!(resolver.cadence(), HostCadence::Hz60);
}

#[test]
fn mixed_samples_near_a_boundary_do_not_flap() {
    let mut resolver = HostCadenceResolver::new(HostCadence::Hz120);

    for index in 0..(HostCadenceResolver::PROMOTION_SAMPLES * 4) {
        let work_micros = if index % 5 == 0 { 4_500 } else { 2_000 };
        sample(&mut resolver, work_micros, false);
        assert_eq!(resolver.cadence(), HostCadence::Hz120);
    }
}
