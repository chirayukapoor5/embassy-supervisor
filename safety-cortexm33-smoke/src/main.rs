//! Executes the actual safety-core Rust policy on QEMU's Arm Cortex-M33.
//! The MPS2-AN505 model is NOT an RP2350 emulator.

#![no_std]
#![no_main]
use cortex_m_rt::entry;
use safety_core::{Inputs, Mode, SafetyConfig, SafetyController};
use safety_core::deadline::{
    DeadlineConfig, DeadlineFault, DeadlineMonitor, DeadlineSnapshot,
    enforced_workload_limit,
};
use safety_core::escalation::{
    EscalationConfig, EscalationPolicy, EscalationReason, EscalationStage,
};

#[entry]
fn main() -> ! {
    if verify_shared_policy() { semihost_exit(0) } else { semihost_exit(1) }
}

fn verify_shared_policy() -> bool {
    let Ok(mut thermal) = SafetyController::new(SafetyConfig::default()) else {
        return false;
    };
    if thermal.mode() != Mode::Safe { return false; }
    for _ in 0..3 {
        thermal.update(Inputs { temperature_c: Some(40), workload_requested: true });
    }
    if thermal.mode() != Mode::Active { return false; }
    let d = thermal.update(Inputs { temperature_c: Some(72), workload_requested: true });
    if d.mode != Mode::Throttled || d.max_workload_percent != 35 { return false; }
    let d = thermal.update(Inputs { temperature_c: Some(85), workload_requested: true });
    if d.mode != Mode::Safe || d.max_workload_percent != 0 { return false; }

    let Ok(mut monitor) = DeadlineMonitor::new(DeadlineConfig::default()) else { return false; };
    monitor.observe(DeadlineSnapshot {
        now_ms: 100, running: true, last_completed_ms: 100,
        completed_windows: 1, missed_windows: 0,
    });
    let report = monitor.observe(DeadlineSnapshot {
        now_ms: 200, running: true, last_completed_ms: 200,
        completed_windows: 2, missed_windows: 1,
    });
    if report.fault != Some(DeadlineFault::MissedPeriod)
        || enforced_workload_limit(100, report.fault) != 0 {
        return false;
    }

    let Ok(mut policy) = EscalationPolicy::new(EscalationConfig::default()) else { return false; };
    let first = policy.observe(1_000, Some(EscalationReason::DeadlineMiss));
    let last = policy.observe(6_000, Some(EscalationReason::DeadlineMiss));
    if first.stage != EscalationStage::Grace
        || last.stage != EscalationStage::ResetPending
        || last.permits_watchdog_feed() {
        return false;
    }
    let later = policy.observe(7_000, None);
    later.stage == EscalationStage::ResetPending && !later.permits_watchdog_feed()
}

/// QEMU-only semihosting SYS_EXIT_EXTENDED, with exit status.
/// Hardware without an attached semihosting host would fault on BKPT.
fn semihost_exit(status: u32) -> ! {
    let params: [u32; 2] = [0x20026, status]; // ADP_Stopped_ApplicationExit
    unsafe {
        core::arch::asm!(
            "bkpt 0xab",
            in("r0") 0x20_u32,
            in("r1") params.as_ptr(),
            options(nostack)
        );
    }
    loop { core::hint::spin_loop(); }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    semihost_exit(2)
}
