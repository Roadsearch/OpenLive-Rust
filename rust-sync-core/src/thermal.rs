//! Pure thermal policy ported from `ThermalGovernor.kt`.
//!
//! Android remains responsible for reading PowerManager and battery broadcasts
//! and for scheduling the 90-second cooldown callback. This state machine
//! computes the target level and applies immediate escalation / one-step easing.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ThermalLevel {
    Normal,
    Light,
    Moderate,
    Severe,
    Critical,
    Emergency,
}

impl ThermalLevel {
    pub const fn bitrate_factor(self) -> f32 {
        match self {
            Self::Normal => 1.0,
            Self::Light => 0.9,
            Self::Moderate => 0.75,
            Self::Severe => 0.55,
            Self::Critical => 0.4,
            Self::Emergency => 0.0,
        }
    }

    pub const fn from_system_status(status: i32) -> Self {
        match status {
            i32::MIN..=0 => Self::Normal,
            1 => Self::Light,
            2 => Self::Moderate,
            3 => Self::Severe,
            4 => Self::Critical,
            _ => Self::Emergency,
        }
    }

    /// Battery temperature is in tenths of a degree Celsius, as Android reports it.
    pub const fn from_battery_tenths_c(temp: i32) -> Self {
        if temp >= 480 {
            Self::Critical
        } else if temp >= 460 {
            Self::Severe
        } else if temp >= 440 {
            Self::Moderate
        } else if temp >= 420 {
            Self::Light
        } else {
            Self::Normal
        }
    }

    pub const fn one_step_cooler(self) -> Self {
        match self {
            Self::Emergency => Self::Critical,
            Self::Critical => Self::Severe,
            Self::Severe => Self::Moderate,
            Self::Moderate => Self::Light,
            Self::Light => Self::Normal,
            Self::Normal => Self::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThermalGovernor {
    level: ThermalLevel,
}

impl Default for ThermalGovernor {
    fn default() -> Self {
        Self {
            level: ThermalLevel::Normal,
        }
    }
}

impl ThermalGovernor {
    pub const COOLDOWN_MS: u64 = 90_000;
    pub const fn level(&self) -> ThermalLevel {
        self.level
    }
    pub const fn bitrate_factor(&self) -> f32 {
        self.level.bitrate_factor()
    }

    /// Escalates immediately. Cooling requires an explicit cooldown tick and
    /// drops exactly one level, matching the Android governor's delayed callback.
    pub fn observe(&mut self, system: ThermalLevel, battery: ThermalLevel) -> ThermalLevel {
        let target = system.max(battery);
        if target > self.level {
            self.level = target;
        }
        self.level
    }

    /// Call only after the Android adapter's 90-second cool-down timer fires.
    pub fn cooldown_tick(&mut self, system: ThermalLevel, battery: ThermalLevel) -> ThermalLevel {
        let target = system.max(battery);
        if target < self.level {
            self.level = self.level.one_step_cooler();
        }
        self.level
    }

    pub fn reset(&mut self) {
        self.level = ThermalLevel::Normal;
    }
}
