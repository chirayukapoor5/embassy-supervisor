use safety_core::{ConfigError, Fault, Inputs, Mode, SafetyConfig, SafetyController, workload_limit};

fn sample(t: Option<i16>, active: bool) -> Inputs {
    Inputs { temperature_c: t, workload_requested: active }
}
fn ready() -> SafetyController {
    let mut c = SafetyController::new(SafetyConfig::default()).unwrap();
    for _ in 0..3 {
        c.update(sample(Some(40), true));
    }
    assert_eq!(c.mode(), Mode::Active);
    c
}

#[test]
fn boots_safe_until_three_consecutive_cool_samples() {
    let mut c = SafetyController::new(SafetyConfig::default()).unwrap();
    assert_eq!(c.mode(), Mode::Safe);
    assert_eq!(c.fault(), Some(Fault::NoValidSample));
    let d = c.update(sample(Some(40), true));
    assert_eq!(d.mode, Mode::Safe);
    assert_eq!(d.recovery_samples_remaining, 2);
    assert!(!d.state_changed);
    assert_eq!(c.update(sample(Some(40), true)).recovery_samples_remaining, 1);
    let d = c.update(sample(Some(40), true));
    assert_eq!(d.mode, Mode::Active);
    assert_eq!(d.max_workload_percent, 100);
    assert!(d.state_changed);
    assert_eq!(d.fault, None);
}

#[test]
fn workload_request_selects_idle_and_active() {
    let mut c = ready();
    let d = c.update(sample(Some(40), false));
    assert_eq!(d.mode, Mode::Idle);
    assert_eq!(d.max_workload_percent, 10);
    assert_eq!(c.update(sample(Some(40), true)).mode, Mode::Active);
}

#[test]
fn throttles_on_high_temperature_and_releases_with_hysteresis() {
    let mut c = ready();
    assert_eq!(c.update(sample(Some(70), true)).mode, Mode::Throttled);
    assert_eq!(c.update(sample(Some(68), true)).mode, Mode::Throttled);
    assert_eq!(c.update(sample(Some(66), true)).max_workload_percent, 35);
    assert_eq!(c.update(sample(Some(65), true)).mode, Mode::Active);
    assert_eq!(c.update(sample(Some(69), true)).mode, Mode::Active);
    assert_eq!(c.update(sample(Some(70), true)).mode, Mode::Throttled);
}

#[test]
fn throttle_has_priority_over_idle_request() {
    let mut c = ready();
    assert_eq!(c.update(sample(Some(72), false)).mode, Mode::Throttled);
    assert_eq!(c.update(sample(Some(60), false)).mode, Mode::Idle);
}

#[test]
fn critical_temperature_immediately_enters_safe() {
    let mut c = ready();
    let d = c.update(sample(Some(85), true));
    assert_eq!(d.mode, Mode::Safe);
    assert_eq!(d.max_workload_percent, 0);
    assert_eq!(d.fault, Some(Fault::CriticalTemperature));
    assert!(d.state_changed);
}

#[test]
fn missing_or_implausible_sensor_readings_fail_closed() {
    let mut c = ready();
    let d = c.update(sample(None, true));
    assert_eq!(d.mode, Mode::Safe);
    assert_eq!(d.fault, Some(Fault::SensorMissing));
    c = ready();
    let d = c.update(sample(Some(126), true));
    assert_eq!(d.mode, Mode::Safe);
    assert_eq!(d.fault, Some(Fault::SensorOutOfRange));
    c = ready();
    assert_eq!(c.update(sample(Some(-41), true)).fault, Some(Fault::SensorOutOfRange));
    c = ready();
    assert_eq!(c.update(sample(Some(0), true)).mode, Mode::Active);
}

#[test]
fn recovery_only_after_three_consecutive_readings_at_or_below_sixty() {
    let mut c = ready();
    c.update(sample(Some(90), true));
    assert_eq!(c.update(sample(Some(60), true)).recovery_samples_remaining, 2);
    assert_eq!(c.update(sample(Some(61), true)).recovery_samples_remaining, 3);
    assert_eq!(c.update(sample(Some(60), true)).recovery_samples_remaining, 2);
    assert_eq!(c.update(sample(Some(59), true)).recovery_samples_remaining, 1);
    let d = c.update(sample(Some(60), true));
    assert_eq!(d.mode, Mode::Active);
    assert_eq!(d.fault, None);
}

#[test]
fn new_fault_resets_recovery_progress() {
    let mut c = ready();
    c.update(sample(Some(85), true));
    c.update(sample(Some(55), true));
    assert_eq!(c.update(sample(None, true)).recovery_samples_remaining, 3);
    assert_eq!(c.update(sample(Some(55), true)).recovery_samples_remaining, 2);
    assert_eq!(c.mode(), Mode::Safe);
}

#[test]
fn recovery_can_enter_idle_if_no_workload_is_requested() {
    let mut c = ready();
    c.update(sample(None, true));
    for _ in 0..3 {
        c.update(sample(Some(35), false));
    }
    assert_eq!(c.mode(), Mode::Idle);
}

#[test]
fn invalid_threshold_configuration_is_rejected() {
    let mut bad = SafetyConfig::default();
    bad.recovery_samples = 0;
    assert!(matches!(SafetyController::new(bad), Err(ConfigError::ZeroRecoverySamples)));
    bad = SafetyConfig::default();
    bad.throttle_off_c = 80;
    assert!(matches!(SafetyController::new(bad), Err(ConfigError::InvalidThresholds)));
}

#[test]
fn safe_workload_is_always_zero() {
    assert_eq!(workload_limit(Mode::Safe), 0);
    assert_eq!(workload_limit(Mode::Throttled), 35);
}

#[test]
fn state_changed_reports_actual_transition_only() {
    let mut c = ready();
    assert!(!c.update(sample(Some(40), true)).state_changed);
    assert!(c.update(sample(Some(72), true)).state_changed);
    assert!(!c.update(sample(Some(73), true)).state_changed);
}
