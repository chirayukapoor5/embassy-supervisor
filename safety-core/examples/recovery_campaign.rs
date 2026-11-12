//! Reproducible host demonstration of diagnostic-gated fault recovery.
//! These are controlled model timestamps, not RP2350 hardware measurements.
use safety_core::diagnostic::{diagnostic_probe_check, run_diagnostic_probe};
use safety_core::deadline::{DeadlineConfig, DeadlineFault, DeadlineMonitor, DeadlineSnapshot};
use safety_core::escalation::{EscalationConfig, EscalationPolicy, EscalationReason};

fn snapshot(now: u32, completed: u32, verified: u32, failures: u32) -> DeadlineSnapshot {
    DeadlineSnapshot {
        now_ms: now, running: true, last_completed_ms: now,
        completed_windows: completed, verified_windows: verified,
        missed_windows: 0, failed_probes: failures,
    }
}

fn main() {
    assert!(run_diagnostic_probe());
    assert!(!diagnostic_probe_check(0x1A2B_3C4C)); // deliberate wrong seed
    let mut monitor = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    let mut escalation = EscalationPolicy::new(EscalationConfig::default()).unwrap();

    let failed = monitor.observe(snapshot(100, 1, 0, 1));
    assert_eq!(failed.fault, Some(DeadlineFault::DiagnosticFailure));
    let grace = escalation.observe(100, Some(EscalationReason::DiagnosticFailure));
    assert!(grace.permits_watchdog_feed());
    println!("t=100ms: injected diagnostic mismatch; fault latched");

    // A worker can return from timers without actually completing the
    // diagnostic; these periods are NOT proof of recovery.
    for (now, completions) in [(200, 2), (300, 3), (400, 4), (500, 5)] {
        let d = monitor.observe(snapshot(now, completions, 0, 1));
        assert_eq!(d.fault, Some(DeadlineFault::DiagnosticFailure));
        assert_eq!(d.recovery_completions_remaining, 3);
    }
    println!("t=500ms: no recovery from timer-only progress");

    // A bounded diagnostic still executes while nonessential app work is
    // gated to zero. Every distinct verified period reduces the countdown.
    for (now, completed, verified, remaining) in [
        (600, 6, 1, 2), (700, 7, 2, 1), (800, 8, 3, 0),
    ] {
        let d = monitor.observe(snapshot(now, completed, verified, 1));
        assert_eq!(d.recovery_completions_remaining, remaining);
    }
    assert_eq!(monitor.fault(), None);
    assert!(escalation.observe(800, None).permits_watchdog_feed());
    println!("t=800ms: three bounded diagnostic completions verified");
    println!("PASS: recoverability backed by verified computation, not timer wakes");
}
