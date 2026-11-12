use embassy_supervisor::TaskNode;

#[cfg(feature = "thermal-demo")]
use core::sync::atomic::{AtomicU32, Ordering};

const SLICE_ITERS: u32 = 50_000;

/// The number of benchmark iterations executed since boot (wraps on overflow).
/// Readable through GET /api/safety when the opt-in demo is enabled.
#[cfg(feature = "thermal-demo")]
pub(crate) static EXECUTED_ITERATIONS: AtomicU32 = AtomicU32::new(0);

/// Run a CPU-bound benchmark task, returning completed nonempty periods.
///
/// Under "thermal-demo", a synthetic-temperature policy sets the per-period
/// iteration cap from another core. SAFE (0%) runs zero iterations, not a
/// spinning loop; shutdown interrupts the 100 ms wait promptly. The quota
/// is *not* a measured CPU-utilization percentage or hardware clock control.
pub(crate) async fn bench_task(node: &'static TaskNode) -> u32 {
    node.set_ready();
    let mut x: u32 = 0x1234_5678;
    let mut slices: u32 = 0;
    loop {
        if node.shutdown_requested() {
            return slices;
        }

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
            EXECUTED_ITERATIONS.fetch_add(iterations, Ordering::Relaxed);
            // The normal benchmark intentionally yields as fast as possible;
            // this version enforces at most one *quota* per 100 ms period.
            // A shutdown request must win over the sleep.
            let _ = embassy_futures::select::select(
                embassy_time::Timer::after_millis(100),
                node.wait_shutdown(),
            )
            .await;
        }
        #[cfg(not(feature = "thermal-demo"))]
        embassy_futures::yield_now().await;
    }
}
