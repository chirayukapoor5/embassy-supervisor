use embassy_futures::select::{Either, select};
use embassy_supervisor::TaskNode;
use embassy_time::{Duration, Instant, Timer};

#[cfg(feature = "thermal-demo")]
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};
#[cfg(feature = "thermal-demo")]
use safety_core::escalation::{
    EscalationConfig, EscalationDecision, EscalationPolicy, EscalationReason, EscalationStage,
};

use crate::GRAPH;

/// Feed period. The bootloader armed the watchdog with an 8 s timeout.
const FEED_EVERY: Duration = Duration::from_secs(2);
const WD_TIMEOUT: Duration = Duration::from_secs(8);

#[cfg(feature = "thermal-demo")]
static ESCALATION_STAGE: AtomicU8 = AtomicU8::new(0);
#[cfg(feature = "thermal-demo")]
static ESCALATION_REASON: AtomicU8 = AtomicU8::new(0);
#[cfg(feature = "thermal-demo")]
static ESCALATION_AGE_MS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "thermal-demo")]
static PREVIOUS_RESET_REASON: AtomicU8 = AtomicU8::new(0);

#[cfg(feature = "thermal-demo")]
pub(crate) fn set_previous_reset(reason: Option<EscalationReason>) {
    PREVIOUS_RESET_REASON.store(reason.map_or(0, EscalationReason::code), Ordering::Release);
}

#[cfg(feature = "thermal-demo")]
pub(crate) fn previous_reset_cause() -> Option<EscalationReason> {
    EscalationReason::from_code(PREVIOUS_RESET_REASON.load(Ordering::Acquire))
}

#[cfg(feature = "thermal-demo")]
pub(crate) fn previous_reset_reason() -> &'static str {
    match previous_reset_cause() {
        Some(reason) => reason.name(),
        None => "none",
    }
}

#[cfg(feature = "thermal-demo")]
pub(crate) fn escalation_stage_name() -> &'static str {
    match ESCALATION_STAGE.load(Ordering::Acquire) {
        1 => "grace",
        2 => "reset-pending",
        _ => "healthy",
    }
}

#[cfg(feature = "thermal-demo")]
pub(crate) fn escalation_reason_name() -> &'static str {
    match EscalationReason::from_code(ESCALATION_REASON.load(Ordering::Acquire)) {
        Some(reason) => reason.name(),
        None => "none",
    }
}

#[cfg(feature = "thermal-demo")]
pub(crate) fn escalation_age_ms() -> u32 {
    ESCALATION_AGE_MS.load(Ordering::Acquire)
}

#[cfg(feature = "thermal-demo")]
fn publish_escalation(d: EscalationDecision) {
    ESCALATION_REASON.store(d.reason.map_or(0, EscalationReason::code), Ordering::Release);
    ESCALATION_AGE_MS.store(d.fault_age_ms, Ordering::Release);
    ESCALATION_STAGE.store(match d.stage {
        EscalationStage::Healthy => 0,
        EscalationStage::Grace => 1,
        EscalationStage::ResetPending => 2,
    }, Ordering::Release);
}

/// POST /api/reboot: the watchdog task resets the chip through the watchdog.
#[cfg(feature = "enabled-if")]
pub(crate) static REBOOT: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    (),
> = embassy_sync::signal::Signal::new();

/// What woke the watchdog task besides its feed deadline.
enum Wake {
    Health(embassy_supervisor::HealthEvent),
    #[cfg(feature = "enabled-if")]
    Reboot,
}

/// Feed the hardware watchdog and detect stalled tasks.
///
/// Opt-in thermal-demo: persistent second-core deadline or monitor-health
/// failure activates a sticky reset decision. Before withholding the feed,
/// preserve the reason in SCRATCH3 and disable BENCH auto-start for the next
/// reboot (when enabled-if is compiled). The existing 8-second watchdog then
/// performs the actual hardware reset. Short faults may recover normally.
pub(crate) async fn watchdog_task(
    node: &'static TaskNode,
    wd: &mut embassy_rp::watchdog::Watchdog,
) {
    node.set_detached(true);
    const STALL_TICKS: u32 = (embassy_time::TICK_HZ / 10) as u32;
    let mut warned = [0u32; 16];

    #[cfg(feature = "thermal-demo")]
    let mut escalation = EscalationPolicy::new(EscalationConfig::default())
        .expect("valid watchdog escalation config");
    #[cfg(feature = "thermal-demo")]
    let mut reset_reason_written = false;

    let mut next_feed = Instant::now();
    loop {
        if Instant::now() >= next_feed {
            next_feed = Instant::now() + FEED_EVERY;

            #[cfg(feature = "thermal-demo")]
            let decision = {
                let now_ms = Instant::now().as_millis() as u32;
                let root_cause = crate::thermal_demo::escalation_cause();
                let decision = escalation.observe(now_ms, root_cause);
                publish_escalation(decision);
                if decision.stage == EscalationStage::ResetPending && !reset_reason_written {
                    if let Some(reason) = decision.reason {
                        crate::bootflag::stash_safety_reset_reason(reason);
                        #[cfg(feature = "enabled-if")]
                        crate::bootflag::set_bench(false);
                        defmt::error!(
                            "watchdog: escalation={} persistent for {} ms; \
                             preserving reason and withholding feed",
                            reason.name(), decision.fault_age_ms
                        );
                    }
                    reset_reason_written = true;
                }
                decision
            };

            #[cfg(feature = "thermal-demo")]
            feed(wd, decision.permits_watchdog_feed());
            #[cfg(not(feature = "thermal-demo"))]
            feed(wd);

            for id in embassy_supervisor::trace::executors() {
                if id == 0 {
                    continue;
                }
                if let Some((stalled, ticks)) =
                    embassy_supervisor::trace::stalled_task(id, STALL_TICKS)
                {
                    defmt::warn!(
                        "trace: {} has been polling for {} ticks",
                        stalled.name(),
                        ticks
                    );
                }
            }
            for (n, w) in GRAPH.nodes.iter().flatten().zip(warned.iter_mut()) {
                let max = n.max_poll_ticks();
                if max > STALL_TICKS && max > *w {
                    *w = max;
                    defmt::warn!("trace: {} once held the executor {} ticks", n.name(), max);
                }
            }
        }

        #[cfg(feature = "breadcrumb")]
        crate::bootflag::stash_crumb(embassy_supervisor::crumb_word());

        let Either::Second(wake) = select(Timer::at(next_feed), next_wake()).await else {
            continue;
        };
        match wake {
            Wake::Health(ev) => match ev.kind {
                embassy_supervisor::HealthKind::Stale { ticks } => defmt::warn!(
                    "liveness: {} stopped beating ({} ticks, still marked running)",
                    ev.node.name(),
                    ticks
                ),
                embassy_supervisor::HealthKind::Recovered => {
                    defmt::info!("liveness: {} is beating again", ev.node.name())
                }
                _ => {}
            },
            #[cfg(feature = "enabled-if")]
            Wake::Reboot => {
                defmt::info!("watchdog: reboot requested");
                wd.trigger_reset();
            }
        }
    }
}

/// Under the existing feed-gate, staleness also suppresses feeds. The new
/// escalation decision is independent of the optional feed-gate flag.
fn feed(
    wd: &mut embassy_rp::watchdog::Watchdog,
    #[cfg(feature = "thermal-demo")] escalation_permits_feed: bool,
) {
    #[cfg(feature = "thermal-demo")]
    if !escalation_permits_feed {
        defmt::error!("watchdog: feed withheld by safety escalation");
        return;
    }
    #[cfg(feature = "feed-gate")]
    if !GRAPH.all_fresh() {
        let stale = GRAPH.stale_nodes().next().map_or("?", |n| n.name());
        defmt::error!("watchdog: feed withheld, stale: {}", stale);
        return;
    }
    wd.feed(WD_TIMEOUT);
}

/// The next health report or reboot request, whichever comes first.
async fn next_wake() -> Wake {
    let wake = async { Wake::Health(embassy_supervisor::wait_health().await) };
    #[cfg(feature = "enabled-if")]
    let wake = async {
        match select(wake, REBOOT.wait()).await {
            Either::First(w) => w,
            Either::Second(()) => Wake::Reboot,
        }
    };
    wake.await
}
