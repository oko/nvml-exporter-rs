use nvml::bitmasks::device::ThrottleReasons;
use nvml::enum_wrappers::device::{Clock, ClockId, EccCounter, MemoryError, MemoryLocation};

pub fn clock_id_str(cid: ClockId) -> &'static str {
    match cid {
        ClockId::Current => "current",
        ClockId::TargetAppClock => "app_clock_target",
        ClockId::DefaultAppClock => "app_clock_default",
        ClockId::CustomerMaxBoost => "customer_boost_max",
    }
}

pub fn clock_type_str(ctype: Clock) -> &'static str {
    match ctype {
        Clock::Graphics => "graphics",
        Clock::SM => "sm",
        Clock::Memory => "mem",
        Clock::Video => "video",
    }
}

pub fn throttle_reason_str(reason: ThrottleReasons) -> &'static str {
    match reason {
        ThrottleReasons::GPU_IDLE => "gpu_idle",
        ThrottleReasons::APPLICATIONS_CLOCKS_SETTING => "applications_clocks_setting",
        ThrottleReasons::SW_POWER_CAP => "sw_power_cap",
        ThrottleReasons::HW_SLOWDOWN => "hw_slowdown",
        ThrottleReasons::SYNC_BOOST => "sync_boost",
        ThrottleReasons::SW_THERMAL_SLOWDOWN => "sw_thermal_slowdown",
        ThrottleReasons::HW_THERMAL_SLOWDOWN => "hw_thermal_slowdown",
        ThrottleReasons::HW_POWER_BRAKE_SLOWDOWN => "hw_power_brake_slowdown",
        ThrottleReasons::DISPLAY_CLOCK_SETTING => "display_clock_setting",
        ThrottleReasons::NONE => "none",
        _ => "unknown",
    }
}

pub fn memory_error_type_str(e: &MemoryError) -> &'static str {
    match e {
        MemoryError::Corrected => "corrected",
        MemoryError::Uncorrected => "uncorrected",
    }
}

pub fn ecc_counter_type_str(c: &EccCounter) -> &'static str {
    match c {
        EccCounter::Aggregate => "aggregate",
        EccCounter::Volatile => "volatile",
    }
}

pub fn memory_location_str(m: &MemoryLocation) -> &'static str {
    match m {
        MemoryLocation::Cbu => "cbu",
        MemoryLocation::Device => "device",
        MemoryLocation::L1Cache => "l1_cache",
        MemoryLocation::L2Cache => "l2_cache",
        MemoryLocation::RegisterFile => "register_file",
        MemoryLocation::Shared => "shared",
        MemoryLocation::SRAM => "sram",
        MemoryLocation::Texture => "texture",
    }
}
