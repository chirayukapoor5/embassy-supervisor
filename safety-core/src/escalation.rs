//! Persistent-fault escalation policy for an existing hardware watchdog.
//!
//! The policy does not manipulate hardware and has no clock dependency. A
//! caller observes a monotonic wrapping millisecond counter and a root cause
//! periodically. Persistent faults progress from Grace -> ResetPending; the
//! terminal state is sticky until reboot. At ResetPending the application
//! must stop feeding its existing hardware watchdog. All timings are demo
//! settings, not a validated automotive safety response time.

use crate::deadline::elapsed_ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscalationReason {
    DeadlineMiss,
    TaskStalled,
    MonitorStale,
    DiagnosticFailure,
}

impl EscalationReason {
    pub const fn code(self) -> u8 {
        match self {
            Self::DeadlineMiss => 1,
            Self::TaskStalled => 2,
            Self::MonitorStale => 3,
            Self::DiagnosticFailure => 4,
        }
    }

    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::DeadlineMiss),
            2 => Some(Self::TaskStalled),
            3 => Some(Self::MonitorStale),
            4 => Some(Self::DiagnosticFailure),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::DeadlineMiss => "deadline-miss",
            Self::TaskStalled => "task-stalled",
            Self::MonitorStale => "monitor-stale",
            Self::DiagnosticFailure => "diagnostic-failure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscalationStage {
    Healthy,
    Grace,
    ResetPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscalationConfigError {
    InvalidGracePeriod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EscalationConfig {
    /// Continuous, same-reason fault age required before withholding a feed.
    /// Must be nonzero and less than half the wrapping u32 millisecond range.
    pub grace_ms: u32,
}

impl Default for EscalationConfig {
    fn default() -> Self {
        Self { grace_ms: 5_000 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EscalationDecision {
    pub stage: EscalationStage,
    pub reason: Option<EscalationReason>,
    pub fault_age_ms: u32,
    pub grace_remaining_ms: u32,
}

impl EscalationDecision {
    /// Escalation never authorizes feeding after ResetPending.
    /// The existing independent liveness gate can also refuse a feed.
    pub const fn permits_watchdog_feed(self) -> bool {
        !matches!(self.stage, EscalationStage::ResetPending)
    }
}

/// The reset decision is a latch; a successful temperature sample or task
/// restart cannot undo a reset once the policy has declared it necessary.
pub struct EscalationPolicy {
    grace_ms: u32,
    stage: EscalationStage,
    reason: Option<EscalationReason>,
    fault_started_ms: u32,
}

impl EscalationPolicy {
    pub fn new(cfg: EscalationConfig) -> Result<Self, EscalationConfigError> {
        if cfg.grace_ms == 0 || cfg.grace_ms >= 0x8000_0000 {
            return Err(EscalationConfigError::InvalidGracePeriod);
        }
        Ok(Self {
            grace_ms: cfg.grace_ms,
            stage: EscalationStage::Healthy,
            reason: None,
            fault_started_ms: 0,
        })
    }

    /// Called from the watchdog feeder at every feed opportunity.
    ///
    /// A transient absence of fault returns to healthy, resetting the grace
    /// clock. A different reason starts a fresh grace window. A reset pending
    /// decision stays latched, even if the observed fault subsequently clears.
    pub fn observe(
        &mut self,
        now_ms: u32,
        fault: Option<EscalationReason>,
    ) -> EscalationDecision {
        if self.stage != EscalationStage::ResetPending {
            match fault {
                None => {
                    self.stage = EscalationStage::Healthy;
                    self.reason = None;
                    self.fault_started_ms = now_ms;
                }
                Some(reason) if self.reason != Some(reason) => {
                    self.reason = Some(reason);
                    self.stage = EscalationStage::Grace;
                    self.fault_started_ms = now_ms;
                }
                Some(_) => {
                    if elapsed_ms(now_ms, self.fault_started_ms) >= self.grace_ms {
                        self.stage = EscalationStage::ResetPending;
                    }
                }
            }
        }

        let elapsed = if self.reason.is_some() {
            elapsed_ms(now_ms, self.fault_started_ms)
        } else {
            0
        };

        EscalationDecision {
            stage: self.stage,
            reason: self.reason,
            fault_age_ms: elapsed,
            grace_remaining_ms: if self.stage == EscalationStage::Grace {
                self.grace_ms.saturating_sub(elapsed)
            } else {
                0
            },
        }
    }
}
