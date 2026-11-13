//! Opt-in wall-clock measurement; run with --release --ignored --nocapture.
use realmkit_engine::{Command, Engine};
use realmkit_spec::{WorldSpec, DURATION_BOUND};
use std::time::Instant;

#[test]
#[ignore = "wall-clock benchmark"]
fn long_advances() {
    for name in ["marches", "turning-ford"] {
        let path = format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"));
        let world = WorldSpec::load(path).unwrap();
        for days in [30, 400] {
            let mut engine = Engine::new_with_seed(&world, 7).unwrap();
            let minutes = days * 1440;
            let mut left = minutes;
            let start = Instant::now();
            while left > 0 {
                let span = left.min(DURATION_BOUND);
                engine.execute(Command::PassTime(span)).unwrap();
                left -= span;
            }
            let seconds = start.elapsed().as_secs_f64();
            eprintln!(
                "{name}: {days} days, {seconds:.6}s, {:.0} minutes/s",
                minutes as f64 / seconds
            );
        }
    }
}
