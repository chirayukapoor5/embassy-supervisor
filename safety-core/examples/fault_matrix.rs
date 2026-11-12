//! Deterministic fault-injection campaign; all cases use actual safety-core.
use safety_core::{Inputs, Mode, SafetyConfig, SafetyController, budgeted_iterations};
use safety_core::deadline::{DeadlineConfig, DeadlineFault, DeadlineMonitor, DeadlineSnapshot, enforced_workload_limit, monitor_fresh};
use safety_core::escalation::{EscalationConfig, EscalationPolicy, EscalationReason as R, EscalationStage as S};

struct Results { total: usize, failures: usize }
impl Results {
    fn new() -> Self { Self { total: 0, failures: 0 } }
    fn case(&mut self, id: &str, requirement: &str, injection: &str, expected: &str, observed: &str) {
        let passed = expected == observed;
        self.total += 1;
        if !passed { self.failures += 1; }
        println!(
            r#"{{"type":"scenario","id":"{}","requirement":"{}","injection":"{}","expected":"{}","observed":"{}","passed":{}}}"#,
            id, requirement, injection, expected, observed, passed,
        );
    }
    fn finish(self) {
        println!(
            r#"{{"type":"summary","scenarios":{},"failed":{},"method":"deterministic-host-model","hardware_executed":false}}"#,
            self.total, self.failures,
        );
        assert_eq!(self.failures, 0, "verification mismatches");
    }
}
fn input(t: Option<i16>, active: bool) -> Inputs {
    Inputs { temperature_c: t, workload_requested: active }
}
fn ready() -> SafetyController {
    let mut c = SafetyController::new(SafetyConfig::default()).unwrap();
    for _ in 0..3 { c.update(input(Some(40), true)); }
    c
}
fn snap(now: u32, running: bool, count: u32, miss: u32, last: u32) -> DeadlineSnapshot {
    DeadlineSnapshot {
        now_ms: now, running, completed_windows: count,
        missed_windows: miss, last_completed_ms: last,
    }
}
fn mode(m: Mode) -> &'static str {
    match m {
        Mode::Active=>"ACTIVE",Mode::Idle=>"IDLE",Mode::Throttled=>"THROTTLED",Mode::Safe=>"SAFE"
    }
}
fn deadline() -> DeadlineMonitor { DeadlineMonitor::new(DeadlineConfig::default()).unwrap() }
fn escalation() -> EscalationPolicy { EscalationPolicy::new(EscalationConfig::default()).unwrap() }
fn stage(s: S) -> &'static str {
    match s { S::Healthy=>"HEALTHY",S::Grace=>"GRACE",S::ResetPending=>"RESET_PENDING" }
}

fn main() {
    let mut r=Results::new();
    let c=SafetyController::new(SafetyConfig::default()).unwrap();
    r.case("FI-001","SR-001","boot_no_sample","SAFE",mode(c.mode()));
    let mut c=ready();
    r.case("FI-002","SR-002","temperature_72","THROTTLED",mode(c.update(input(Some(72),true)).mode));
    let mut c=ready();
    r.case("FI-003","SR-003","temperature_85","SAFE",mode(c.update(input(Some(85),true)).mode));
    let mut c=ready();
    r.case("FI-004","SR-004","sensor_disconnected","SAFE",mode(c.update(input(None,true)).mode));
    let mut c=ready();
    r.case("FI-005","SR-005","sensor_out_of_range_126","SAFE",mode(c.update(input(Some(126),true)).mode));
    let mut c=ready();
    c.update(input(Some(85),true));
    c.update(input(Some(55),true));
    c.update(input(Some(55),true));
    let before=mode(c.mode());
    c.update(input(Some(55),true));
    let after=mode(c.mode());
    r.case("FI-006","SR-006","two_then_three_safe_samples","SAFE_to_ACTIVE",
           if before=="SAFE"&&after=="ACTIVE"{"SAFE_to_ACTIVE"}else{"INCORRECT"});
    r.case("FI-007","SR-007","throttle_35_percent_of_50000","17500",
           if budgeted_iterations(50_000,35)==17_500{"17500"}else{"INCORRECT"});
    let mut m=deadline();
    m.observe(snap(100,true,1,0,100));
    let d=m.observe(snap(200,true,2,1,200));
    r.case("FI-008","SR-008","missed_period_counter","MissedPeriod",
           if d.fault==Some(DeadlineFault::MissedPeriod){"MissedPeriod"}else{"INCORRECT"});
    let mut m=deadline();
    m.observe(snap(100,true,1,0,100));
    let d=m.observe(snap(351,true,1,0,100));
    r.case("FI-009","SR-009","no_progress_251ms","TaskStalled",
           if d.fault==Some(DeadlineFault::TaskStalled){"TaskStalled"}else{"INCORRECT"});
    let mut m=deadline();
    m.observe(snap(100,true,1,1,100));
    let no_progress=m.observe(snap(150,true,1,1,100));
    m.observe(snap(200,true,2,1,200));
    m.observe(snap(300,true,3,1,300));
    let recovered=m.observe(snap(400,true,4,1,400));
    r.case("FI-010","SR-010","duplicate_poll_then_three_completions","LATCHED_then_CLEAR",
           if no_progress.fault.is_some()&&recovered.fault.is_none(){"LATCHED_then_CLEAR"}else{"INCORRECT"});
    r.case("FI-011","SR-011","missing_monitor_stamp","BLOCKED",
           if !monitor_fresh(300,0,250){"BLOCKED"}else{"INCORRECT"});
    let mut p=escalation();
    let begin=p.observe(1_000,Some(R::DeadlineMiss));
    let prior=p.observe(5_999,Some(R::DeadlineMiss));
    r.case("FI-012","SR-012","continuous_fault_4999ms","GRACE",
           if begin.stage==S::Grace{stage(prior.stage)}else{"INCORRECT"});
    let mut p=escalation();
    p.observe(1_000,Some(R::TaskStalled));
    let d=p.observe(6_000,Some(R::TaskStalled));
    r.case("FI-013","SR-013","continuous_fault_5000ms","RESET_PENDING",stage(d.stage));
    let d=p.observe(6_100,None);
    r.case("FI-014","SR-014","fault_disappeared_after_escalation","BLOCK_FEED",
           if !d.permits_watchdog_feed(){"BLOCK_FEED"}else{"INCORRECT"});
    let code=R::MonitorStale.code();
    r.case("FI-015","SR-015","diagnostic_code_round_trip","MonitorStale",
           if R::from_code(code)==Some(R::MonitorStale){"MonitorStale"}else{"INCORRECT"});
    let mut p=escalation();
    let d=p.observe(10_000,None);
    r.case("FI-016","SR-016","thermal_only_safe_no_deadline_issue","HEALTHY",stage(d.stage));
    let enforced=enforced_workload_limit(100,Some(DeadlineFault::TaskStalled));
    r.case("FI-017","SR-017","deadline_overrides_active","ZERO",
           if enforced==0{"ZERO"}else{"INCORRECT"});
    let mut p=escalation();
    let start=u32::MAX-1_000;
    p.observe(start,Some(R::MonitorStale));
    let d=p.observe(start.wrapping_add(5_000),Some(R::MonitorStale));
    r.case("FI-018","SR-018","timer_counter_rollover","RESET_PENDING",stage(d.stage));
    r.finish();
}
