use stellar_devkit::harness::{load_all, load_scenario, scenario_names};
use stellar_devkit::sandbox::fixtures::{
    burst, fees, fixture, flat, mean_fee, peak_fee, ramp, random_walk, spikes,
};
use stellar_devkit::types::FeeRecord;

fn assert_well_formed(records: &[FeeRecord]) {
    assert!(!records.is_empty(), "series is empty");
    for record in records {
        assert!(record.fee_charged <= record.max_fee, "overpaying record");
        assert!(record.fee_charged >= 0, "negative fee");
    }
    for pair in records.windows(2) {
        assert!(
            pair[0].ledger < pair[1].ledger,
            "ledgers are not consecutive"
        );
    }
}

#[test]
fn every_generator_produces_a_well_formed_series() {
    assert_well_formed(&flat(8, 100, 100, 100));
    assert_well_formed(&ramp(8, 100, 100, 800, 1000));
    assert_well_formed(&spikes(8, 100, 100, 900, 3));
    assert_well_formed(&burst(8, 100, 100, 5000, 2, 5));
    assert_well_formed(&random_walk(8, 100, 100, 20, 21));
}

#[test]
fn the_generators_honour_the_requested_length() {
    for len in [1usize, 2, 3, 17, 64] {
        assert_eq!(flat(len, 0, 100, 100).len(), len);
        assert_eq!(ramp(len, 0, 100, 900, 1000).len(), len);
        assert_eq!(spikes(len, 0, 100, 900, 2).len(), len);
        assert_eq!(burst(len, 0, 100, 900, 1, 3).len(), len);
        assert_eq!(random_walk(len, 0, 100, 20, 8).len(), len);
    }
}

#[test]
fn a_generated_ramp_is_monotonic() {
    let records = ramp(12, 0, 100, 1300, 2000);
    for pair in records.windows(2) {
        assert!(pair[1].fee_charged >= pair[0].fee_charged);
    }
}

#[test]
fn a_generated_spike_series_oscillates() {
    let records = spikes(10, 0, 100, 900, 3);
    assert_eq!(peak_fee(&records), 900);
    assert!(fees(&records).contains(&100));
    assert!(mean_fee(&records) > 100.0 && mean_fee(&records) < 900.0);
}

#[test]
fn a_generated_burst_is_confined_to_its_window() {
    let records = burst(20, 0, 100, 5000, 8, 12);
    let elevated = records.iter().filter(|r| r.fee_charged == 5000).count();
    assert_eq!(elevated, 4);
    assert!(records
        .iter()
        .all(|r| r.fee_charged == 100 || r.fee_charged == 5000));
}

#[test]
fn generated_series_replay_for_the_same_seed() {
    for seed in [1u64, 99, 123456789] {
        let first = random_walk(32, 0, 250, 40, seed);
        let second = random_walk(32, 0, 250, 40, seed);
        assert_eq!(first, second, "seed {seed} did not replay");
    }
}

#[test]
fn a_generated_fixture_matches_the_harness_shape() {
    let generated = fixture(
        "generated-spike",
        "programmatically generated",
        spikes(9, 900, 100, 700, 4),
    );
    let loaded = load_scenario("spike").expect("fixture loads");
    assert!(!loaded.records.is_empty());
    assert_eq!(
        generated.records.first().map(|record| record.ledger),
        Some(900)
    );
    assert_eq!(generated.record_count(), 9);
    assert_eq!(generated.max_fee_charged(), 700);
    assert!(generated
        .records
        .iter()
        .any(|record| record.fee_charged == 700));
    assert!(generated
        .records
        .iter()
        .all(|record| record.fee_charged == 100 || record.fee_charged == 700));
}

#[test]
fn generated_series_satisfy_the_loader_invariants() {
    for records in [
        flat(5, 0, 100, 100),
        ramp(5, 0, 100, 500, 1000),
        spikes(5, 0, 100, 900, 2),
        burst(5, 0, 100, 5000, 1, 3),
        random_walk(5, 0, 100, 15, 77),
    ] {
        let scenario = fixture("generated", "checked against the loader", records);
        assert!(!scenario.records.is_empty());
        assert!(scenario.description.len() > 4);
        assert!(scenario.ledger_range().is_some());
        assert!(scenario.min_fee_charged() <= scenario.max_fee_charged());
        for record in &scenario.records {
            assert!(record.fee_charged <= record.max_fee);
        }
    }
}

#[test]
fn the_shipped_fixtures_all_load_and_describe_themselves() {
    assert_eq!(load_all().expect("fixtures load").len(), scenario_names().len());
    for name in scenario_names() {
        let scenario = load_scenario(name).expect("fixture loads");
        assert_eq!(&scenario.name, name);
        assert!(scenario.record_count() > 0);
        assert!(!scenario.description.is_empty());
    }
}

#[test]
fn a_zero_length_generated_fixture_is_reported_as_empty() {
    let scenario = fixture("empty", "nothing", Vec::new());
    assert_eq!(scenario.record_count(), 0);
    assert_eq!(scenario.ledger_range(), None);
    assert_eq!(mean_fee(&scenario.records), 0.0);
    assert_eq!(peak_fee(&scenario.records), 0);
}
