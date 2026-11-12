use safety_core::diagnostic::{diagnostic_probe_check, run_diagnostic_probe};
use safety_core::escalation::EscalationReason as R;
use safety_core::reset_record::{
    allow_bench_autostart, decode_reset_reason, encode_reset_reason,
};

#[test]
fn golden_diagnostic_vector_runs_real_bounded_work() {
    assert!(run_diagnostic_probe());
    assert!(diagnostic_probe_check(0x1A2B_3C4D));
}

#[test]
fn changed_diagnostic_input_is_detected() {
    assert!(!diagnostic_probe_check(0x1A2B_3C4C));
    assert!(!diagnostic_probe_check(0));
}

#[test]
fn each_fault_reason_survives_record_roundtrip() {
    for reason in [R::DeadlineMiss, R::TaskStalled, R::MonitorStale] {
        assert_eq!(decode_reset_reason(encode_reset_reason(reason)), Some(reason));
    }
}

#[test]
fn reset_records_refuse_corruption_unknown_or_erased_data() {
    let encoded = encode_reset_reason(R::TaskStalled);
    assert_eq!(decode_reset_reason(encoded ^ 1), None);
    assert_eq!(decode_reset_reason(encoded ^ (1 << 8)), None);
    assert_eq!(decode_reset_reason(encoded ^ (1 << 16)), None);
    assert_eq!(decode_reset_reason(0), None);
    assert_eq!(decode_reset_reason(u32::MAX), None);
    assert_eq!(decode_reset_reason(0xD15A_00FF), None); // unknown code
}

#[test]
fn previous_escalation_blocks_bench_autostart() {
    assert!(allow_bench_autostart(true, None));
    assert!(!allow_bench_autostart(false, None));
    for reason in [R::DeadlineMiss, R::TaskStalled, R::MonitorStale] {
        assert!(!allow_bench_autostart(true, Some(reason)));
    }
}
