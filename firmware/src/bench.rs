use embassy_supervisor::TaskNode;

#[cfg(feature = "thermal-demo")]
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const SLICE_ITERS: u32 = 50_000;
#[cfg(feature = "thermal-demo")]
const PERIOD_MS: u64 = 100;

/// Cumulative benchmark iterations (wrapping). No actual utilization claim.
#[cfg(feature = "thermal-demo")]
pub(crate) static EXECUTED_ITERATIONS: AtomicU32 = AtomicU32::new(0);

/// Monitored second-core progress, observable from the safety monitor on core 0.
#[cfg(feature = "thermal-demo")]
pub(crate) static BENCH_RUNNING: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "thermal-demo")]
pub(crate) static LAST_COMPLETED_MS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "thermal-demo")]
pub(crate) static COMPLETED_WINDOWS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "thermal-demo")]
pub(crate) static MISSED_WINDOWS: AtomicU32 = AtomicU32::new(0);

#[cfg(feature = "thermal-demo")]
pub(crate) fn snapshot() -> safety_core::deadline::DeadlineSnapshot {
    use embassy_time::Instant;
    safety_core::deadline::DeadlineSnapshot {
        // Snapshot the worker atomics before the monitor's time.
        // A concurrent later completion must not look 49 days old.
        running: BENCH_RUNNING.load(Ordering::Acquire),
        last_completed_ms: LAST_COMPLETED_MS.load(Ordering::Acquire),
        completed_windows: COMPLETED_WINDOWS.load(Ordering::Acquire),
        missed_windows: MISSED_WINDOWS.load(Ordering::Acquire),
        now_ms: Instant::now().as_millis() as u32,
    }
}

/// Run the CPU benchmark. The opt-in thermal-demo measures each 100 ms work
/// period, publishes completion counts and gates new work when the deadline
/// monitor or thermal policy fails. The upstream default remains unchanged.
pub(crate) async fn bench_task(node: &'static TaskNode) -> u32 {
    node.set_ready();
    let mut x: u32 = 0x1234_5678;
    let mut slices: u32 = 0;

    #[cfg(feature = "thermal-demo")]
    {
        LAST_COMPLETED_MS.store(embassy_time::Instant::now().as_millis() as u32, Ordering::Release);
        BENCH_RUNNING.store(true, Ordering::Release);
    }

    loop {
        if node.shutdown_requested() {
            #[cfg(feature = "thermal-demo")]
            BENCH_RUNNING.store(false, Ordering::Release);
            return slices;
        }

        #[cfg(feature = "thermal-demo")]
        let window_start = embassy_time::Instant::now();

        #[cfg(feature = "thermal-demo")]
        let iterations = safety_core::budgeted_iterations(
            SLICE_ITERS,
            crate::thermal_demo::workload_limit(),
        );
        #[cfg(not(feature = "thermal-demo"))]
        let iterations = SLICE_ITERS;

        for _ in 0..iterations {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
        }
        if iterations > 0 {
            core::hint::black_box(x);
            slices = slices.wrapping_add(1);
        }

        #[cfg(feature = "thermal-demo")]
        {
            use embassy_futures::select::{Either, select};
            use embassy_time::{Duration, Instant, Timer};

            EXECUTED_ITERATIONS.fetch_add(iterations, Ordering::Relaxed);
            let due = window_start + Duration::from_millis(PERIOD_MS);
            // A shutdown wins over waiting for the next period.
            match select(Timer::at(due), node.wait_shutdown()).await {
                Either::Second(()) => {
                    BENCH_RUNNING.store(false, Ordering::Release);
                    return slices;
                }
                Either::First(()) => {}
            }

            let completed_at = Instant::now();
            // Measure actual period completion, including timing jitter.
            if completed_at > due {
                // Timer scheduling is approximate: tolerate 5 ms before
                // recording a miss rather than counting sub-ms wake jitter.
                if completed_at - due > Duration::from_millis(5) {
                    MISSED_WINDOWS.fetch_add(1, Ordering::AcqRel);
                }
            }
            LAST_COMPLETED_MS.store(completed_at.as_millis() as u32, Ordering::Release);
            COMPLETED_WINDOWS.fetch_add(1, Ordering::AcqRel);
        }
        #[cfg(not(feature = "thermal-demo"))]
        embassy_futures::yield_now().await;
    }
}
