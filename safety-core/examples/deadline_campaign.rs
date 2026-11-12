//! Reproducible host-only deadline fault campaign (no hardware attached).
use safety_core::deadline::{
    DeadlineConfig, DeadlineFault, DeadlineMonitor, DeadlineSnapshot, enforced_workload_limit,
};

fn observe(
    monitor: &mut DeadlineMonitor,
    tag: &str,
    now_ms: u32,
    running: bool,
    completed_at: u32,
    completed: u32,
    missed: u32,
    expected: Option<DeadlineFault>,
) {
    let report = monitor.observe(DeadlineSnapshot {
        now_ms,
        running,
        last_completed_ms: completed_at,
        completed_windows: completed,
        missed_windows: missed,
    });
    let cap = enforced_workload_limit(100, report.fault);
    assert_eq!(report.fault, expected, "scenario {tag}");
    assert_eq!(cap, if expected.is_some() { 0 } else { 100 });
    println!(
        "{tag:27} t={now_ms:4}ms running={running} completions={completed:2} \
         misses={missed} fault={:?} recovery_remaining={} workload_cap={cap}%",
        report.fault,
        report.recovery_completions_remaining,
    );
}

fn main() {
    println!("DETERMINISTIC HOST FAULT CAMPAIGN — simulated timestamps and counters only");
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    observe(&mut m, "normal", 100, true, 100, 1, 0, None);
    observe(&mut m, "deadline overrun injected", 200, true, 200, 2, 1, Some(DeadlineFault::MissedPeriod));
    observe(&mut m, "repeat observation", 205, true, 200, 2, 1, Some(DeadlineFault::MissedPeriod));
    observe(&mut m, "recovery beat 1", 300, true, 300, 3, 1, Some(DeadlineFault::MissedPeriod));
    observe(&mut m, "recovery beat 2", 400, true, 400, 4, 1, Some(DeadlineFault::MissedPeriod));
    observe(&mut m, "recovery beat 3", 500, true, 500, 5, 1, None);
    observe(&mut m, "stall injected", 751, true, 500, 5, 1, Some(DeadlineFault::TaskStalled));
    observe(&mut m, "stopped, still latched", 800, false, 500, 5, 1, Some(DeadlineFault::TaskStalled));
    observe(&mut m, "stall recovery beat 1", 900, true, 900, 6, 1, Some(DeadlineFault::TaskStalled));
    observe(&mut m, "stall recovery beat 2", 1000, true, 1000, 7, 1, Some(DeadlineFault::TaskStalled));
    observe(&mut m, "stall recovery beat 3", 1100, true, 1100, 8, 1, None);
    println!("PASS: every scenario matched the expected safety outcome");
}
