//! Deadline/freshness verification for a periodic, cooperative benchmark.
//!
//! This is a host-testable state machine, not an RTOS hard-deadline guarantee.
//! The caller is responsible for measuring work windows and publishing atomic
//! progress counters from the worker. A monitor runs independently of the worker.

/// A detected fault from an expected, running periodic workload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineFault {
    /// At least one completed period exceeded its deadline.
    MissedPeriod,
    /// No completed period arrived within the freshness timeout.
    TaskStalled,
}

/// Observer inputs. All millisecond values are relative monotonic timestamps
/// represented as wrapping u32; call observe at least once every 2^31 ms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlineSnapshot {
    pub now_ms: u32,
    pub running: bool,
    pub last_completed_ms: u32,
    pub completed_windows: u32,
    pub missed_windows: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlineConfig {
    /// Maximum time since a completion before the task is considered stalled.
    /// Choose this greater than the task period plus its permitted jitter.
    pub stale_after_ms: u32,
    /// Number of DISTINCT new, non-overrunning work completions required after
    /// a fault before it can be cleared.
    pub recovery_completions: u8,
}

impl Default for DeadlineConfig {
    fn default() -> Self {
        Self { stale_after_ms: 250, recovery_completions: 3 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineConfigError {
    InvalidTimeout,
    InvalidRecoveryCount,
}

impl DeadlineConfig {
    pub fn validate(self) -> Result<(), DeadlineConfigError> {
        if self.stale_after_ms == 0 || self.stale_after_ms >= 0x8000_0000 {
            return Err(DeadlineConfigError::InvalidTimeout);
        }
        if self.recovery_completions == 0 {
            return Err(DeadlineConfigError::InvalidRecoveryCount);
        }
        Ok(())
    }
}

/// Current monitored status; no fault means the deadline gate is open, subject
/// to independent freshness of the monitoring task itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlineReport {
    pub fault: Option<DeadlineFault>,
    pub completion_age_ms: u32,
    pub recovery_completions_remaining: u8,
}

/// A non-allocating monitor with a fault latch and evidence-based recovery.
///
/// A missed period or stale task closes the gate immediately. Three DISTINCT
/// fresh, successful completions reopen it. Monitoring an inactive workload
/// does not cause false trips; inactivity does NOT clear a previous fault.
pub struct DeadlineMonitor {
    cfg: DeadlineConfig,
    fault: Option<DeadlineFault>,
    last_seen_completion: u32,
    last_seen_miss: u32,
    good_after_fault: u8,
}

impl DeadlineMonitor {
    pub fn new(cfg: DeadlineConfig) -> Result<Self, DeadlineConfigError> {
        cfg.validate()?;
        Ok(Self {
            cfg,
            fault: None,
            last_seen_completion: 0,
            last_seen_miss: 0,
            good_after_fault: 0,
        })
    }

    pub const fn fault(&self) -> Option<DeadlineFault> { self.fault }

    pub fn observe(&mut self, s: DeadlineSnapshot) -> DeadlineReport {
        let fresh_completion = s.completed_windows != self.last_seen_completion;
        let new_miss = s.missed_windows != self.last_seen_miss;
        self.last_seen_completion = s.completed_windows;
        self.last_seen_miss = s.missed_windows;
        let age = elapsed_ms(s.now_ms, s.last_completed_ms);

        if s.running {
            if new_miss {
                self.trip(DeadlineFault::MissedPeriod);
            } else if age > self.cfg.stale_after_ms {
                self.trip(DeadlineFault::TaskStalled);
            } else if self.fault.is_some() && fresh_completion {
                self.good_after_fault = self.good_after_fault.saturating_add(1);
                if self.good_after_fault >= self.cfg.recovery_completions {
                    self.fault = None;
                    self.good_after_fault = 0;
                }
            }
        }

        DeadlineReport {
            fault: self.fault,
            completion_age_ms: age,
            recovery_completions_remaining: if self.fault.is_some() {
                self.cfg.recovery_completions.saturating_sub(self.good_after_fault)
            } else {
                0
            },
        }
    }

    fn trip(&mut self, fault: DeadlineFault) {
        self.fault = Some(fault);
        self.good_after_fault = 0;
    }
}

/// Wrap-safe age calculation for a contemporaneous timestamp pair.
///
/// If \`stamp\` is slightly newer than \`now\` due to concurrent sampling,
/// do not interpret it as 49 days old. The caller must observe within 2^31 ms.
pub const fn elapsed_ms(now_ms: u32, stamp_ms: u32) -> u32 {
    let delta = now_ms.wrapping_sub(stamp_ms);
    if delta >= 0x8000_0000 { 0 } else { delta }
}

/// Independent fail-closed freshness gate for the safety monitor itself.
/// On startup (stamp == 0), no workload is authorized.
pub const fn monitor_fresh(now_ms: u32, stamp_ms: u32, timeout_ms: u32) -> bool {
    stamp_ms != 0 && elapsed_ms(now_ms, stamp_ms) <= timeout_ms
}

/// Enforce the more restrictive of thermal policy and deadline supervision.
pub const fn enforced_workload_limit(thermal_percent: u8, fault: Option<DeadlineFault>) -> u8 {
    if fault.is_some() { 0 } else if thermal_percent > 100 { 100 } else { thermal_percent }
}
