//! Opt-in, synthetic thermal scenario feeding the real RP2350 bench task.
//!
//! NO physical sensor is read. This scenario is intentionally separate from
//! any production thermal-safety implementation. It is for demonstration,
//! cross-core integration and future verification work only.

use core::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use embassy_time::Timer;
use safety_core::{Fault, Inputs, Mode, SafetyConfig, SafetyController};

const SENSOR_MISSING: i32 = i32::MIN;

// Fail closed on startup: no benchmark work is allowed until the safety
// controller has received enough valid simulated cool samples.
static WORKLOAD_LIMIT: AtomicU8 = AtomicU8::new(0);
static LAST_TEMPERATURE: AtomicI32 = AtomicI32::new(SENSOR_MISSING);

/// Cross-core publication from core 0 to core 1.
pub(crate) fn workload_limit() -> u8 {
    WORKLOAD_LIMIT.load(Ordering::Acquire)
}

pub(crate) fn simulated_temperature() -> Option<i32> {
    let value = LAST_TEMPERATURE.load(Ordering::Acquire);
    if value == SENSOR_MISSING {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn mode_name() -> &'static str {
    match workload_limit() {
        100 => "ACTIVE",
        10 => "IDLE",
        35 => "THROTTLED",
        _ => "SAFE",
    }
}

fn name(mode: Mode) -> &'static str {
    match mode {
        Mode::Active => "ACTIVE",
        Mode::Idle => "IDLE",
        Mode::Throttled => "THROTTLED",
        Mode::Safe => "SAFE",
    }
}

fn fault_name(fault: Option<Fault>) -> &'static str {
    match fault {
        None => "none",
        Some(Fault::NoValidSample) => "no-valid-sample",
        Some(Fault::SensorMissing) => "sensor-missing",
        Some(Fault::SensorOutOfRange) => "sensor-out-of-range",
        Some(Fault::CriticalTemperature) => "critical-temperature",
    }
}

/// Scripted (temperature °C, workload requested) scenarios. Each point is
/// published at a 2-second cadence; repeat the sequence indefinitely.
const SCENARIO: &[(Option<i16>, bool)] = &[
    (Some(40), true),  // SAFE: recovery sample 1
    (Some(40), true),  // SAFE: recovery sample 2
    (Some(40), true),  // ACTIVE
    (Some(72), true),  // THROTTLED
    (Some(78), true),  // THROTTLED
    (Some(85), true),  // SAFE: emergency
    (Some(55), true),  // SAFE: recovery sample 1
    (Some(55), true),  // SAFE: recovery sample 2
    (Some(55), true),  // ACTIVE: recovery sample 3
    (Some(40), false), // IDLE
    (Some(40), true),  // ACTIVE
    (None, true),      // SAFE: disconnected sensor
    (Some(40), true),
    (Some(40), true),
    (Some(40), true),  // ACTIVE
];

#[embassy_executor::task]
pub(crate) async fn thermal_demo_task() {
    let mut controller = SafetyController::new(SafetyConfig::default())
        .expect("validated synthetic safety policy");
    let mut index = 0usize;
    loop {
        let (temperature_c, workload_requested) = SCENARIO[index];
        index = (index + 1) % SCENARIO.len();

        let decision = controller.update(Inputs {
            temperature_c,
            workload_requested,
        });
        LAST_TEMPERATURE.store(
            temperature_c.map_or(SENSOR_MISSING, i32::from),
            Ordering::Release,
        );
        WORKLOAD_LIMIT.store(decision.max_workload_percent, Ordering::Release);

        defmt::info!(
            "THERMAL-DEMO (synthetic): temperature={}C present={} state={} \
             benchmark_budget={} percent fault={} recovery_remaining={}",
            temperature_c.unwrap_or(0),
            temperature_c.is_some(),
            name(decision.mode),
            decision.max_workload_percent,
            fault_name(decision.fault),
            decision.recovery_samples_remaining,
        );
        Timer::after_secs(2).await;
    }
}
