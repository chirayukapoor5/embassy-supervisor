use safety_core::{Inputs, SafetyConfig, SafetyController};

fn main() {
    let mut controller = SafetyController::new(SafetyConfig::default())
        .expect("default configuration must be valid");

    // Demonstration inputs ONLY: not a live RP2350 sensor or hardware tests.
    let sequence = [
        (Some(40), true),
        (Some(40), true),
        (Some(40), true),
        (Some(72), true),
        (Some(80), true),
        (Some(85), true),
        (Some(75), true),
        (Some(55), true),
        (Some(55), true),
        (Some(55), true),
        (None, true),
        (Some(40), false),
        (Some(40), false),
        (Some(40), false),
    ];

    println!("SIMULATED thermal safety policy; no hardware is controlled");
    for (i, (temperature_c, workload_requested)) in sequence.into_iter().enumerate() {
        let decision = controller.update(Inputs {
            temperature_c,
            workload_requested,
        });
        println!(
            "sample={:02} temp={:?}C work={} mode={:?} limit={}%% fault={:?} remaining={}",
            i + 1,
            temperature_c,
            workload_requested,
            decision.mode,
            decision.max_workload_percent,
            decision.fault,
            decision.recovery_samples_remaining,
        );
    }
}
