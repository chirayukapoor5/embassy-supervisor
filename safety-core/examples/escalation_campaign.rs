//! Hardware-free scripted watchdog escalation test. No reset is issued here.
use safety_core::escalation::{
    EscalationConfig, EscalationPolicy, EscalationReason as R, EscalationStage as S,
};

fn main() {
    let mut p = EscalationPolicy::new(EscalationConfig::default()).unwrap();
    let steps = [
        (0, None, S::Healthy),
        (2_000, Some(R::TaskStalled), S::Grace),
        (4_000, Some(R::TaskStalled), S::Grace),
        (6_000, None, S::Healthy),
        (8_000, Some(R::DeadlineMiss), S::Grace),
        (10_000, Some(R::DeadlineMiss), S::Grace),
        (12_000, Some(R::DeadlineMiss), S::Grace),
        (14_000, Some(R::DeadlineMiss), S::ResetPending),
        (16_000, None, S::ResetPending),
    ];
    println!("WATCHDOG POLICY CAMPAIGN: host simulation, no RP2350 reset");
    for (now_ms, fault, expected) in steps {
        let decision = p.observe(now_ms, fault);
        assert_eq!(decision.stage, expected, "t={now_ms}ms");
        println!(
            "t={now_ms:5}ms fault={:?} phase={:?} age={}ms remaining={}ms feed={}",
            fault, decision.stage, decision.fault_age_ms,
            decision.grace_remaining_ms, decision.permits_watchdog_feed()
        );
    }
    println!("PASS: transient recovery, persistent escalation and sticky reset");
}
