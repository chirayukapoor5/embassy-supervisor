//! Deterministic, hardware-independent thermal safety policy.
//!
//! This crate deliberately has no HAL, OS, heap, timer, or hardware dependency.
//! Call [SafetyController::update] once per validated sampling opportunity.
//! The caller must provide fresh sensor data and enforce the returned workload
//! limit in real firmware. The temperature thresholds are illustrative and are
//! NOT validated RP2350 specifications or automotive safety limits.
#![no_std]

/// The four supported operating states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal operation; up to 100% of configured workload.
    Active,
    /// No active work; reduced background workload.
    Idle,
    /// Temperature is elevated; cap application workload.
    Throttled,
    /// Unsafe or unknown condition; application workload must be disabled.
    Safe,
}

/// Reason for entering or remaining in the fail-safe state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// No validated temperature sample has been received since power-up.
    NoValidSample,
    /// The sensor supplied no reading.
    SensorMissing,
    /// The sensor reading is outside the configured plausible range.
    SensorOutOfRange,
    /// Temperature reached or exceeded the critical threshold.
    CriticalTemperature,
}

/// Temperature values are whole degrees Celsius to avoid floating point.
///
/// All values are simulation/demo defaults rather than device specifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafetyConfig {
    pub min_valid_c: i16,
    pub max_valid_c: i16,
    pub throttle_on_c: i16,
    pub throttle_off_c: i16,
    pub critical_c: i16,
    pub safe_recovery_c: i16,
    /// Number of consecutive valid, sufficiently cool readings before SAFE
    /// may be exited. The caller owns the sampling interval and freshness.
    pub recovery_samples: u8,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            min_valid_c: -40,
            max_valid_c: 125,
            throttle_on_c: 70,
            throttle_off_c: 65,
            critical_c: 85,
            safe_recovery_c: 60,
            recovery_samples: 3,
        }
    }
}

/// The configuration cannot provide safe, unambiguous transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    InvalidThresholds,
    ZeroRecoverySamples,
}

impl SafetyConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.recovery_samples == 0 {
            return Err(ConfigError::ZeroRecoverySamples);
        }
        if !(self.min_valid_c < self.safe_recovery_c
            && self.safe_recovery_c <= self.throttle_off_c
            && self.throttle_off_c < self.throttle_on_c
            && self.throttle_on_c < self.critical_c
            && self.critical_c <= self.max_valid_c)
        {
            return Err(ConfigError::InvalidThresholds);
        }
        Ok(())
    }
}

/// Inputs received at a single sampling opportunity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inputs {
    /// None represents a missing/failed sensor reading.
    pub temperature_c: Option<i16>,
    /// True when application work is requested.
    pub workload_requested: bool,
}

/// Safety decision to be enforced by the surrounding firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub mode: Mode,
    /// Maximum application workload, in percent; not a hardware clock setting.
    pub max_workload_percent: u8,
    pub fault: Option<Fault>,
    pub state_changed: bool,
    /// Number of additional consecutive cool, valid samples needed to leave SAFE.
    pub recovery_samples_remaining: u8,
}

/// Persistent safety policy state. Start in SAFE until good data arrives.
#[derive(Debug, Clone, Copy)]
pub struct SafetyController {
    config: SafetyConfig,
    mode: Mode,
    fault: Option<Fault>,
    good_recovery_samples: u8,
}

impl SafetyController {
    pub fn new(config: SafetyConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        Ok(Self {
            config,
            mode: Mode::Safe,
            fault: Some(Fault::NoValidSample),
            good_recovery_samples: 0,
        })
    }

    pub const fn mode(&self) -> Mode {
        self.mode
    }

    pub const fn fault(&self) -> Option<Fault> {
        self.fault
    }

    /// Calculate the next state from one fresh sensor sample.
    ///
    /// Priority: missing/implausible/critical sample -> SAFE; safe recovery
    /// requires N consecutive cool samples; otherwise apply throttling
    /// hysteresis and activity-based ACTIVE/IDLE selection.
    pub fn update(&mut self, inputs: Inputs) -> Decision {
        let previous = self.mode;
        match inputs.temperature_c {
            None => self.trip(Fault::SensorMissing),
            Some(t) if t < self.config.min_valid_c || t > self.config.max_valid_c => {
                self.trip(Fault::SensorOutOfRange)
            }
            Some(t) if t >= self.config.critical_c => {
                self.trip(Fault::CriticalTemperature)
            }
            Some(t) if self.mode == Mode::Safe => {
                if t <= self.config.safe_recovery_c {
                    self.good_recovery_samples = self.good_recovery_samples.saturating_add(1);
                    if self.good_recovery_samples >= self.config.recovery_samples {
                        self.good_recovery_samples = 0;
                        self.fault = None;
                        self.mode = if inputs.workload_requested {
                            Mode::Active
                        } else {
                            Mode::Idle
                        };
                    }
                } else {
                    self.good_recovery_samples = 0;
                }
            }
            Some(t) => {
                self.fault = None;
                self.good_recovery_samples = 0;
                self.mode = if t >= self.config.throttle_on_c
                    || (self.mode == Mode::Throttled && t > self.config.throttle_off_c)
                {
                    Mode::Throttled
                } else if inputs.workload_requested {
                    Mode::Active
                } else {
                    Mode::Idle
                };
            }
        }

        Decision {
            mode: self.mode,
            max_workload_percent: workload_limit(self.mode),
            fault: self.fault,
            state_changed: previous != self.mode,
            recovery_samples_remaining: if self.mode == Mode::Safe {
                self.config
                    .recovery_samples
                    .saturating_sub(self.good_recovery_samples)
            } else {
                0
            },
        }
    }

    fn trip(&mut self, reason: Fault) {
        self.mode = Mode::Safe;
        self.fault = Some(reason);
        self.good_recovery_samples = 0;
    }
}

/// Policy recommendation only; an application must actually enforce it.
pub const fn workload_limit(mode: Mode) -> u8 {
    match mode {
        Mode::Active => 100,
        Mode::Idle => 10,
        Mode::Throttled => 35,
        Mode::Safe => 0,
    }
}
