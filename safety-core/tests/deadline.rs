use safety_core::deadline::{
    DeadlineConfig, DeadlineConfigError, DeadlineFault, DeadlineMonitor, DeadlineSnapshot,
    elapsed_ms, enforced_workload_limit, monitor_fresh,
};

fn snapshot(now: u32, running: bool, last: u32, completions: u32, missed: u32) -> DeadlineSnapshot {
    DeadlineSnapshot {
        now_ms: now,
        running,
        last_completed_ms: last,
        completed_windows: completions,
        verified_windows: completions,
        missed_windows: missed,
        failed_probes: 0,
    }
}

#[test]
fn idle_worker_is_not_a_false_deadline_fault() {
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    assert_eq!(m.observe(snapshot(10_000, false, 0, 0, 0)).fault, None);
}

#[test]
fn healthy_periods_keep_gate_open() {
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    for n in 1..8 {
        assert_eq!(m.observe(snapshot(n * 100, true, n * 100, n, 0)).fault, None);
    }
}

#[test]
fn explicit_deadline_miss_trips_and_does_not_self_clear_on_polls() {
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    m.observe(snapshot(100, true, 100, 1, 0));
    let r = m.observe(snapshot(200, true, 200, 2, 1));
    assert_eq!(r.fault, Some(DeadlineFault::MissedPeriod));
    assert_eq!(r.recovery_completions_remaining, 3);
    for _ in 0..4 {
        assert_eq!(
            m.observe(snapshot(205, true, 200, 2, 1)).fault,
            Some(DeadlineFault::MissedPeriod)
        );
    }
    for (time, count, remaining) in [(300, 3, 2), (400, 4, 1)] {
        let r = m.observe(snapshot(time, true, time, count, 1));
        assert_eq!(r.fault, Some(DeadlineFault::MissedPeriod));
        assert_eq!(r.recovery_completions_remaining, remaining);
    }
    assert_eq!(m.observe(snapshot(500, true, 500, 5, 1)).fault, None);
}

#[test]
fn stalled_task_trips_and_recovers_only_with_new_progress() {
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    m.observe(snapshot(1000, true, 1000, 10, 0));
    assert_eq!(m.observe(snapshot(1251, true, 1000, 10, 0)).fault,
               Some(DeadlineFault::TaskStalled));
    assert_eq!(m.observe(snapshot(1300, false, 1000, 10, 0)).fault,
               Some(DeadlineFault::TaskStalled));
    assert_eq!(m.observe(snapshot(1400, true, 1400, 11, 0)).recovery_completions_remaining, 2);
    assert_eq!(m.observe(snapshot(1500, true, 1500, 12, 0)).recovery_completions_remaining, 1);
    assert_eq!(m.observe(snapshot(1600, true, 1600, 13, 0)).fault, None);
}

#[test]
fn repeat_misses_restart_recovery() {
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    m.observe(snapshot(100, true, 100, 1, 1));
    assert_eq!(m.observe(snapshot(200, true, 200, 2, 1)).recovery_completions_remaining, 2);
    assert_eq!(m.observe(snapshot(300, true, 300, 3, 2)).recovery_completions_remaining, 3);
    assert_eq!(m.fault(), Some(DeadlineFault::MissedPeriod));
}

#[test]
fn stopped_task_does_not_erase_existing_fault() {
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    m.observe(snapshot(100, true, 100, 1, 1));
    for _ in 0..10 {
        assert_eq!(m.observe(snapshot(1000, false, 100, 1, 1)).fault,
                   Some(DeadlineFault::MissedPeriod));
    }
}

#[test]
fn monitor_clock_rollover_and_concurrent_timestamp_are_handled() {
    assert_eq!(elapsed_ms(5, u32::MAX - 4), 10);
    assert_eq!(elapsed_ms(999, 1000), 0); // concurrent newer stamp
    assert!(monitor_fresh(5, u32::MAX - 4, 250));
    assert!(!monitor_fresh(5, u32::MAX - 4, 9));
    assert!(!monitor_fresh(1000, 0, 250));
    assert!(!monitor_fresh(2000, 1000, 250));
}

#[test]
fn a_deadline_fault_overrides_even_active_thermal_policy() {
    assert_eq!(enforced_workload_limit(100, Some(DeadlineFault::TaskStalled)), 0);
    assert_eq!(enforced_workload_limit(35, Some(DeadlineFault::MissedPeriod)), 0);
    assert_eq!(enforced_workload_limit(10, None), 10);
    assert_eq!(enforced_workload_limit(200, None), 100);
}

#[test]
fn monitor_configuration_rejects_missing_recovery_and_bad_timeout() {
    let err = DeadlineMonitor::new(DeadlineConfig {
        stale_after_ms: 0,
        recovery_completions: 3,
    });
    assert!(matches!(err, Err(DeadlineConfigError::InvalidTimeout)));
    let err = DeadlineMonitor::new(DeadlineConfig {
        stale_after_ms: 250,
        recovery_completions: 0,
    });
    assert!(matches!(err, Err(DeadlineConfigError::InvalidRecoveryCount)));
}


#[test]
fn zero_application_windows_without_verified_probes_cannot_clear_latch() {
    use safety_core::deadline::counter_advanced;
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    let mut first = snapshot(100, true, 100, 1, 1);
    first.verified_windows = 0;
    m.observe(first);
    for n in 2..10 {
        let mut s = snapshot(n * 100, true, n * 100, n, 1);
        s.verified_windows = 0;
        let d = m.observe(s);
        assert_eq!(d.fault, Some(DeadlineFault::MissedPeriod));
        assert_eq!(d.recovery_completions_remaining, 3);
    }
    assert!(counter_advanced(0, u32::MAX)); // real wrap
    assert!(!counter_advanced(0, 1)); // unexpected regression
}

#[test]
fn diagnostic_verified_safe_periods_can_release_the_latch() {
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    let mut first = snapshot(100, true, 100, 1, 1);
    first.verified_windows = 0;
    m.observe(first);
    for (n, remaining) in [(2, 2), (3, 1)] {
        let mut s = snapshot(n * 100, true, n * 100, n, 1);
        s.verified_windows = n - 1;
        assert_eq!(m.observe(s).recovery_completions_remaining, remaining);
    }
    let mut last = snapshot(400, true, 400, 4, 1);
    last.verified_windows = 3;
    assert_eq!(m.observe(last).fault, None);
}

#[test]
fn diagnostic_failure_latches_and_repeated_timer_wakes_do_not_clear() {
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    m.observe(snapshot(100, true, 100, 1, 0));
    let mut failing = snapshot(200, true, 200, 2, 0);
    failing.failed_probes = 1;
    failing.verified_windows = 1;
    assert_eq!(m.observe(failing).fault, Some(DeadlineFault::DiagnosticFailure));
    for n in 3..8 {
        let mut s = snapshot(n * 100, true, n * 100, n, 0);
        s.failed_probes = 1;
        s.verified_windows = 1;
        assert_eq!(m.observe(s).fault, Some(DeadlineFault::DiagnosticFailure));
    }
}

#[test]
fn a_second_diagnostic_failure_resets_partial_recovery() {
    let mut m = DeadlineMonitor::new(DeadlineConfig::default()).unwrap();
    let mut first = snapshot(100, true, 100, 1, 0);
    first.failed_probes = 1;
    first.verified_windows = 0;
    m.observe(first);
    let mut good = snapshot(200, true, 200, 2, 0);
    good.failed_probes = 1;
    good.verified_windows = 1;
    assert_eq!(m.observe(good).recovery_completions_remaining, 2);
    let mut fail_again = snapshot(300, true, 300, 3, 0);
    fail_again.failed_probes = 2;
    fail_again.verified_windows = 1;
    let d = m.observe(fail_again);
    assert_eq!(d.fault, Some(DeadlineFault::DiagnosticFailure));
    assert_eq!(d.recovery_completions_remaining, 3);
}
