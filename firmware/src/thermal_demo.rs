//! Simulated temperatures + second-core deadline supervision.
//!
//! Uses a 50-ms best-effort monitor cadence and a 2-second scripted thermal
//! sample cadence. NO physical sensor, safety certification or hard real-time
//! guarantee is implied.

use core::sync::atomic::{AtomicI32, AtomicU8, AtomicU32, Ordering};
use embassy_time::{Instant, Timer};
use safety_core::{Fault, Inputs, Mode, SafetyConfig, SafetyController};
use safety_core::deadline::{
    DeadlineConfig, DeadlineFault, DeadlineMonitor, elapsed_ms, monitor_fresh,
};

const SENSOR_MISSING: i32 = i32::MIN;
const MONITOR_POLL_MS: u64 = 50;
const TEMP_SAMPLE_POLLS: u8 = 40; // 2 seconds
const MONITOR_TIMEOUT_MS: u32 = 250;

// Fail closed until both the thermal policy and monitor have published.
static THERMAL_LIMIT: AtomicU8 = AtomicU8::new(0);
static LAST_TEMPERATURE: AtomicI32 = AtomicI32::new(SENSOR_MISSING);
static DEADLINE_FAULT: AtomicU8 = AtomicU8::new(0);
static RECOVERY_REMAINING: AtomicU8 = AtomicU8::new(0);
static MONITOR_LAST_TICK_MS: AtomicU32 = AtomicU32::new(0);

fn fault_code(fault: Option<DeadlineFault>) -> u8 {
    match fault {
        None => 0,
        Some(DeadlineFault::MissedPeriod) => 1,
        Some(DeadlineFault::TaskStalled) => 2,
    }
}

pub(crate) fn deadline_fault_name() -> &'static str {
    match DEADLINE_FAULT.load(Ordering::Acquire) {
        1 => "missed-period",
        2 => "task-stalled",
        _ => "none",
    }
}

pub(crate) fn deadline_recovery_remaining() -> u8 {
    RECOVERY_REMAINING.load(Ordering::Acquire)
}

pub(crate) fn monitor_is_fresh() -> bool {
    let now = Instant::now().as_millis() as u32;
    monitor_fresh(
        now,
        MONITOR_LAST_TICK_MS.load(Ordering::Acquire),
        MONITOR_TIMEOUT_MS,
    )
}

/// A stale safety monitoring task always closes the second core's workload
/// gate, even if the latest synthetic temperature was previously normal.
pub(crate) fn workload_limit() -> u8 {
    if !monitor_is_fresh() || DEADLINE_FAULT.load(Ordering::Acquire) != 0 {
        0
    } else {
        THERMAL_LIMIT.load(Ordering::Acquire)
    }
}

pub(crate) fn simulated_temperature() -> Option<i32> {
    let value = LAST_TEMPERATURE.load(Ordering::Acquire);
    if value == SENSOR_MISSING { None } else { Some(value) }
}

pub(crate) fn mode_name() -> &'static str {
    if workload_limit() == 0 { return "SAFE"; }
    match THERMAL_LIMIT.load(Ordering::Acquire) {
        100 => "ACTIVE",
        10 => "IDLE",
        35 => "THROTTLED",
        _ => "SAFE",
    }
}

fn thermal_mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Active => "ACTIVE",
        Mode::Idle => "IDLE",
        Mode::Throttled => "THROTTLED",
        Mode::Safe => "SAFE",
    }
}

fn thermal_fault_name(fault: Option<Fault>) -> &'static str {
    match fault {
        None => "none",
        Some(Fault::NoValidSample) => "no-valid-sample",
        Some(Fault::SensorMissing) => "sensor-missing",
        Some(Fault::SensorOutOfRange) => "sensor-out-of-range",
        Some(Fault::CriticalTemperature) => "critical-temperature",
    }
}

fn deadline_name(fault: Option<DeadlineFault>) -> &'static str {
    match fault {
        None => "none",
        Some(DeadlineFault::MissedPeriod) => "missed-period",
        Some(DeadlineFault::TaskStalled) => "task-stalled",
    }
}

/// Synthetic temperature data ONLY, never read from an RP2350 sensor.
const SCENARIO: &[(Option<i16>, bool)] = &[
    (Some(40), true), (Some(40), true), (Some(40), true),
    (Some(72), true), (Some(78), true), (Some(85), true),
    (Some(55), true), (Some(55), true), (Some(55), true),
    (Some(40), false), (Some(40), true), (None, true),
    (Some(40), true), (Some(40), true), (Some(40), true),
];

#[embassy_executor::task]
pub(crate) async fn thermal_demo_task() {
    let mut controller = SafetyController::new(SafetyConfig::default())
        .expect("synthetic thermal config must validate");
    let mut deadlines = DeadlineMonitor::new(DeadlineConfig::default())
        .expect("deadline monitor config must validate");
    let mut sample_index = 0usize;
    let mut polls_until_sample = 0u8;
    let mut previous_fault = None;

    loop {
        if polls_until_sample == 0 {
            let (temperature_c, workload_requested) = SCENARIO[sample_index];
            sample_index = (sample_index + 1) % SCENARIO.len();
            polls_until_sample = TEMP_SAMPLE_POLLS;

            let decision = controller.update(Inputs {
                temperature_c, workload_requested,
            });
            LAST_TEMPERATURE.store(
                temperature_c.map_or(SENSOR_MISSING, i32::from),
                Ordering::Release,
            );
            THERMAL_LIMIT.store(decision.max_workload_percent, Ordering::Release);
            defmt::info!(
                "THERMAL-DEMO (synthetic): temperature={}C present={} state={} \
                 budget={} percent fault={} recovery_remaining={}",
                temperature_c.unwrap_or(0),
                temperature_c.is_some(),
                thermal_mode_name(decision.mode),
                decision.max_workload_percent,
                thermal_fault_name(decision.fault),
                decision.recovery_samples_remaining,
            );
        }

        // Independently monitor the second core. A stale/missed completion
        // immediately gates application work (before another temp sample).
        let report = deadlines.observe(crate::bench::snapshot());
        RECOVERY_REMAINING.store(report.recovery_completions_remaining, Ordering::Release);
        DEADLINE_FAULT.store(fault_code(report.fault), Ordering::Release);
        // Publish freshness last, after the latest decision is committed.
        MONITOR_LAST_TICK_MS.store(Instant::now().as_millis() as u32, Ordering::Release);

        if previous_fault != report.fault {
            defmt::warn!(
                "DEADLINE: fault={} age_ms={} recovery_remaining={} bench_running={}",
                deadline_name(report.fault),
                report.completion_age_ms,
                report.recovery_completions_remaining,
                crate::bench::BENCH_RUNNING.load(Ordering::Acquire),
            );
            previous_fault = report.fault;
        }

        polls_until_sample -= 1;
        Timer::after_millis(MONITOR_POLL_MS).await;
    }
}

/// Status helper for the HTTP dashboard. The time field is best-effort and
/// wrapping; reported independently from the worker progress counters.
pub(crate) fn completion_age_ms() -> Option<u32> {
    let s = crate::bench::snapshot();
    if s.completed_windows == 0 { None }
    else { Some(elapsed_ms(s.now_ms, s.last_completed_ms)) }
}
