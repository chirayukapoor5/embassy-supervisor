use safety_core::escalation::{
    EscalationConfig, EscalationConfigError, EscalationPolicy,
    EscalationReason as R, EscalationStage as S,
};

fn policy() -> EscalationPolicy {
    EscalationPolicy::new(EscalationConfig { grace_ms: 5_000 }).unwrap()
}

#[test]
fn healthy_system_allows_feeds_indefinitely() {
    let mut p = policy();
    for t in [0, 2_000, 8_000, 99_000] {
        let d = p.observe(t, None);
        assert_eq!(d.stage, S::Healthy);
        assert!(d.permits_watchdog_feed());
    }
}

#[test]
fn grace_period_then_reset_on_persistent_deadline_failure() {
    let mut p = policy();
    let d = p.observe(1_000, Some(R::DeadlineMiss));
    assert_eq!(d.stage, S::Grace);
    assert_eq!(d.grace_remaining_ms, 5_000);
    assert!(d.permits_watchdog_feed());
    assert_eq!(p.observe(5_999, Some(R::DeadlineMiss)).stage, S::Grace);
    let d = p.observe(6_000, Some(R::DeadlineMiss));
    assert_eq!(d.stage, S::ResetPending);
    assert_eq!(d.reason, Some(R::DeadlineMiss));
    assert!(!d.permits_watchdog_feed());
}

#[test]
fn transient_failure_clears_before_grace_expires() {
    let mut p = policy();
    p.observe(100, Some(R::TaskStalled));
    assert_eq!(p.observe(4_000, None).stage, S::Healthy);
    assert_eq!(p.observe(5_000, Some(R::TaskStalled)).grace_remaining_ms, 5_000);
    assert_eq!(p.observe(6_000, Some(R::TaskStalled)).stage, S::Grace);
}

#[test]
fn a_different_fault_starts_a_new_grace_window() {
    let mut p = policy();
    p.observe(1_000, Some(R::DeadlineMiss));
    p.observe(5_000, Some(R::TaskStalled));
    let d = p.observe(6_000, Some(R::TaskStalled));
    assert_eq!(d.stage, S::Grace);
    assert_eq!(d.fault_age_ms, 1_000);
    assert_eq!(p.observe(10_000, Some(R::TaskStalled)).stage, S::ResetPending);
}

#[test]
fn reset_pending_remains_latched_even_if_fault_disappears() {
    let mut p = policy();
    p.observe(10, Some(R::MonitorStale));
    p.observe(5_010, Some(R::MonitorStale));
    for now in [5_100, 6_000, 12_000] {
        let d = p.observe(now, None);
        assert_eq!(d.stage, S::ResetPending);
        assert_eq!(d.reason, Some(R::MonitorStale));
        assert!(!d.permits_watchdog_feed());
    }
}

#[test]
fn monotonic_timer_rollover_preserves_fault_age() {
    let mut p = policy();
    let start = u32::MAX - 1_000;
    p.observe(start, Some(R::TaskStalled));
    let d = p.observe(start.wrapping_add(5_000), Some(R::TaskStalled));
    assert_eq!(d.stage, S::ResetPending);
    assert_eq!(d.fault_age_ms, 5_000);
}

#[test]
fn invalid_grace_config_is_refused() {
    assert!(matches!(
        EscalationPolicy::new(EscalationConfig { grace_ms: 0 }),
        Err(EscalationConfigError::InvalidGracePeriod)
    ));
    assert!(matches!(
        EscalationPolicy::new(EscalationConfig { grace_ms: 0x8000_0000 }),
        Err(EscalationConfigError::InvalidGracePeriod)
    ));
}

#[test]
fn reason_encoding_roundtrips_and_rejects_unknown_values() {
    for r in [R::DeadlineMiss, R::TaskStalled, R::MonitorStale, R::DiagnosticFailure] {
        assert_eq!(R::from_code(r.code()), Some(r));
        assert!(!r.name().is_empty());
    }
    assert_eq!(R::from_code(0), None);
    assert_eq!(R::from_code(254), None);
}

#[test]
fn transient_thermal_safe_state_has_no_escalation_reason() {
    // The integration intentionally passes None for thermal-only protection:
    // a scripted high temperature pauses work but does not cause reset storms.
    let mut p = policy();
    for t in [0, 2000, 4000, 6000, 8000] {
        assert_eq!(p.observe(t, None).stage, S::Healthy);
    }
}
