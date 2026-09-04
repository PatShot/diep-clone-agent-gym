//! The shipped configuration file, and the defaults it mirrors.

use spawn::config::{FOCUS_NEST, FOCUS_NEST_ALPHA, FOCUS_SCATTER};
use spawn::SpawnConfig;

fn shipped() -> SpawnConfig {
    SpawnConfig::from_path("../../config/spawn.toml").expect("config/spawn.toml parses")
}

/// The test the loader exists for.
///
/// A configuration file and a set of defaults that drift apart silently are worse
/// than having no file at all: every run would use values nobody read. This fails
/// the build the moment they disagree, down to the last float.
#[test]
fn the_shipped_file_matches_the_shipped_defaults() {
    assert_eq!(shipped(), SpawnConfig::default());
}

#[test]
fn a_configuration_survives_a_round_trip_through_toml() {
    let original = SpawnConfig::default();
    let text = original.to_toml_string().expect("serializes");
    let back = SpawnConfig::from_toml_str(&text).expect("parses");
    assert_eq!(back, original);
}

/// Order is part of the contract, not an accident of how the file was typed. The
/// alpha focus is confined to the smallest region on the map and must ask for ground
/// before anything else claims it.
#[test]
fn the_alpha_focus_is_served_first() {
    let c = shipped();
    let order: Vec<_> = c.foci.iter().map(|f| f.id).collect();
    assert_eq!(order[0], FOCUS_NEST_ALPHA, "alphas must be placed first");
    assert_eq!(order[1], FOCUS_NEST);
    assert_eq!(order[2], FOCUS_SCATTER);
    assert_eq!(order.len(), 5, "scatter, nest, alpha and two wings");
}

#[test]
fn a_bad_file_is_an_error_rather_than_a_panic() {
    assert!(SpawnConfig::from_toml_str("place_attempts = 'not a number'").is_err());
    assert!(SpawnConfig::from_path("../../config/does-not-exist.toml").is_err());
}
