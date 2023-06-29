extern crate nvml_wrapper as nvml;

use std::net::SocketAddr;
use std::str::FromStr;
use std::string::ToString;
use std::sync::Arc;
use std::time::SystemTime;

use clap::Arg;
use clap::ArgAction;
use clap::Command;
use futures::channel::oneshot;
use futures::channel::oneshot::Receiver;
use futures::channel::oneshot::Sender;
use log::debug;
use log::error;
use log::info;
use log::trace;
use log::warn;
use nvml::bitmasks::device::ThrottleReasons;
use nvml::enum_wrappers::device::Clock;
use nvml::enum_wrappers::device::ClockId;
use nvml::enum_wrappers::device::EccCounter;
use nvml::enum_wrappers::device::EncoderType;
use nvml::enum_wrappers::device::MemoryError;
use nvml::enum_wrappers::device::MemoryLocation;
use nvml::enum_wrappers::device::TemperatureSensor;
use nvml::error::NvmlError;
use nvml::Nvml;
use prometheus::default_registry;
use prometheus::register_gauge;
use prometheus::register_gauge_vec;
use prometheus::Encoder;
use prometheus::Gauge;
use prometheus::GaugeVec;
use prometheus::TextEncoder;
use stderrlog;
use tokio::task::JoinSet;
use warp::Filter;

use crate::str_helpers::*;

mod str_helpers;

pub fn server_setup(args: Vec<String>) -> (Vec<(SocketAddr, Receiver<()>)>, Vec<Sender<()>>, Options) {
    let matches = Command::new("nvml-exporter-rs")
        .version("0.0.1")
        .about("Prometheus exporter for NVIDIA GPU NVML metrics")
        .arg(
            Arg::new("listen")
                .short('l')
                .long("listen")
                .value_name("SOCKET_ADDRESS")
                .help("listen address")
                .action(ArgAction::Append)
                .default_values(&["[::]:9996", "0.0.0.0:9996"]),
        )
        .arg(Arg::new("throttle-reasons").long("throttle-reasons").action(ArgAction::SetTrue))
        .arg(Arg::new("verbosity").short('v').action(ArgAction::Count))
        .get_matches_from(args);

    let verbosity = matches.get_count("verbosity");
    stderrlog::new().module(module_path!()).module("nvml_exporter").verbosity(verbosity as usize).show_module_names(true).init().unwrap();

    let addrs = matches.get_many::<String>("listen").unwrap().map(|val| val.to_string()).collect::<Vec<_>>();

    let mut senders: Vec<Sender<()>> = vec![];

    let binds = addrs
        .iter()
        .map(|addr| {
            let (s, r) = oneshot::channel::<()>();
            senders.push(s);
            (SocketAddr::from_str(addr.clone().as_str()).unwrap(), r)
        })
        .collect::<Vec<_>>();

    let opts = Options {
        enable_throttle_reasons: matches.get_flag("throttle-reasons"),
    };

    (binds, senders, opts)
}

#[derive(Copy, Clone)]
pub struct Options {
    enable_throttle_reasons: bool,
}

struct Context {
    metrics: Metrics,
    nvml: Nvml,
    opts: Options,
}

pub async fn serve(binds: Vec<(SocketAddr, Receiver<()>)>, opts: Options) {
    let ctx = Arc::new(Context {
        metrics: Metrics::new().unwrap(),
        nvml: Nvml::init().unwrap(),
        opts: opts.clone(),
    });

    let mut set = JoinSet::new();
    binds.into_iter().for_each(|(addr, recv)| {
        let ctx = ctx.clone();
        let routes = warp::any().map(move || {
            let ctx = ctx.clone();
            match gather(ctx) {
                Ok(_) => (),
                Err(e) => error!("error gathering metrics: {:?}", e),
            }
            let mut buffer = vec![];
            let encoder = TextEncoder::new();
            let metric_families = default_registry().gather();
            encoder.encode(&metric_families, &mut buffer).unwrap();

            String::from_utf8(buffer).unwrap()
        });

        let (addr, server) = warp::serve(routes).bind_with_graceful_shutdown(addr, async move {
            recv.await.ok();
            warn!("gracefully shutting down exporter on {}", addr.to_string());
        });
        info!("starting server on {}", addr);
        set.spawn(server);
    });

    let mut results: Vec<_> = Vec::with_capacity(set.len());
    while let Some(res) = set.join_next().await {
        results.push(res);
    }
    for res in results.iter() {
        match res {
            Ok(_) => (),
            Err(e) => error!("error during server shutdown: {}", e),
        }
    }
}

#[derive(Clone)]
struct Metrics {
    g_device_count: Gauge,
    gv_device_temp: GaugeVec,
    gv_device_power_usage: GaugeVec,
    gv_fbc_stats_sessions_count: GaugeVec,
    gv_fbc_stats_average_fps: GaugeVec,
    gv_fbc_stats_average_latency: GaugeVec,
    gv_running_compute_processes_count: GaugeVec,
    gv_running_graphics_processes_count: GaugeVec,
    gv_current_pcie_link_width: GaugeVec,
    gv_current_pcie_link_generation: GaugeVec,
    gv_max_pcie_link_width: GaugeVec,
    gv_max_pcie_link_generation: GaugeVec,
    gv_utilization_gpu: GaugeVec,
    gv_utilization_memory: GaugeVec,
    gv_clock: GaugeVec,
    gv_applications_clock: GaugeVec,
    gv_memory_info: GaugeVec,
    gv_display_active: GaugeVec,
    gv_display_mode: GaugeVec,
    gv_encoder_capacity_h264: GaugeVec,
    gv_encoder_capacity_hevc: GaugeVec,
    gv_encoder_stats_sessions_count: GaugeVec,
    gv_encoder_stats_average_fps: GaugeVec,
    gv_encoder_stats_average_latency: GaugeVec,
    gv_current_clocks_throttle_reasons: GaugeVec,
    gv_memory_error_counters: GaugeVec,
}

impl Metrics {
    fn new() -> prometheus::Result<Metrics> {
        let dl = &["device", "uuid"];
        Ok(Metrics {
            g_device_count: register_gauge!("nvml_device_count", "number of nvml devices")?,
            gv_device_temp: register_gauge_vec!("nvml_temperature", "temperature of nvml device", dl)?,
            gv_device_power_usage: register_gauge_vec!("nvml_power_usage", "power usage of nvml device", dl)?,
            gv_fbc_stats_sessions_count: register_gauge_vec!("nvml_fbc_stats_sessions_count", "session count for frame buffer capture sessions", dl)?,
            gv_fbc_stats_average_fps: register_gauge_vec!("nvml_fbc_stats_average_fps", "average fps for frame buffer capture sessions", dl)?,
            gv_fbc_stats_average_latency: register_gauge_vec!("nvml_fbc_stats_average_latency", "average latency for frame buffer capture sessions", dl)?,
            gv_running_compute_processes_count: register_gauge_vec!("nvml_running_compute_processes_count", "number of running compute processes", dl)?,
            gv_running_graphics_processes_count: register_gauge_vec!("nvml_running_graphics_processes_count", "number of running graphics processes", dl)?,
            gv_current_pcie_link_width: register_gauge_vec!("nvml_current_pcie_link_width", "current pcie link width", dl)?,
            gv_current_pcie_link_generation: register_gauge_vec!("nvml_current_pcie_link_generation", "current pcie link generation", dl)?,
            gv_max_pcie_link_width: register_gauge_vec!("nvml_max_pcie_link_width", "max pcie link width", dl)?,
            gv_max_pcie_link_generation: register_gauge_vec!("nvml_max_pcie_link_generation", "max pcie link generation", dl)?,
            gv_utilization_gpu: register_gauge_vec!("nvml_utilization_gpu", "GPU utilization", dl)?,
            gv_utilization_memory: register_gauge_vec!("nvml_utilization_memory", "memory utilization", dl)?,
            gv_clock: register_gauge_vec!("nvml_clock", "clock speed", &["device", "uuid", "clock_id", "type"])?,
            gv_applications_clock: register_gauge_vec!("nvml_applications_clock", "clock speed", &["device", "uuid", "clock_id", "type"])?,
            gv_memory_info: register_gauge_vec!("nvml_memory_info", "memory information", &["device", "uuid", "state"])?,
            gv_display_active: register_gauge_vec!("nvml_display_active", "display active", dl)?,
            gv_display_mode: register_gauge_vec!("nvml_display_mode", "display mode", dl)?,
            gv_encoder_capacity_h264: register_gauge_vec!("nvml_encoder_capacity_h264", "encoder capacity", dl)?,
            gv_encoder_capacity_hevc: register_gauge_vec!("nvml_encoder_capacity_hevc", "encoder capacity", dl)?,
            gv_encoder_stats_sessions_count: register_gauge_vec!("nvml_encoder_stats_sessions_count", "session count for encoder sessions", dl)?,
            gv_encoder_stats_average_fps: register_gauge_vec!("nvml_encoder_stats_average_fps", "average fps for encoder sessions", dl)?,
            gv_encoder_stats_average_latency: register_gauge_vec!("nvml_encoder_stats_average_latency", "average latency for encoder sessions", dl)?,
            gv_current_clocks_throttle_reasons: register_gauge_vec!("nvml_current_clocks_throttle_reasons", "current clock throttling reason code", &["device", "uuid", "reason"])?,
            gv_memory_error_counters: register_gauge_vec!("nvml_memory_error_counters", "memory error counters", &["device", "uuid", "mem_error", "ecc_counter", "mem_location"])?,
        })
    }
}

fn gather(ctx: Arc<Context>) -> Result<(), NvmlError> {
    let now = SystemTime::now();
    debug!("starting NVML gather at {}", chrono::Utc::now().format("%c").to_string());

    let count = ctx.nvml.device_count()?;
    ctx.metrics.g_device_count.set(count as f64);

    for device_index in 0..count {
        let device = ctx.nvml.device_by_index(device_index)?;
        let dev_idx_string = device_index.to_string();
        let dev_idx_str = dev_idx_string.as_str();
        let dev_uuid_string = device.uuid()?;
        let dev_uuid = dev_uuid_string.as_str();

        let dl = &[dev_idx_str, dev_uuid];
        macro_rules! set_gv {
            ( $member:expr, $labels:expr, $val:expr ) => {
                $member.with_label_values($labels).set($val as f64);
                if cfg!(debug_assertions) {
                    trace!(stringify!($member));
                }
            };
        }

        #[cfg(debug_assertions)]
        macro_rules! timed {
            ( $n:expr, $e:expr ) => {
                let now = SystemTime::now();
                let _ = $e;
                trace!("{}: took {}ms", $n, now.elapsed().unwrap().as_millis());
            };
        }
        #[cfg(not(debug_assertions))]
        macro_rules! timed {
            ( $n:expr, $e:expr ) => {
                $e
            };
        }

        timed!("core", {
            set_gv!(ctx.metrics.gv_device_temp, dl, device.temperature(TemperatureSensor::Gpu)? as f64);
            set_gv!(ctx.metrics.gv_device_power_usage, dl, (device.power_usage()? as f64) / 1000.);
            set_gv!(ctx.metrics.gv_running_compute_processes_count, dl, device.running_compute_processes_count()? as f64);
            set_gv!(ctx.metrics.gv_running_graphics_processes_count, dl, device.running_graphics_processes_count()? as f64);
            set_gv!(ctx.metrics.gv_current_pcie_link_width, dl, device.current_pcie_link_width()? as f64);
            set_gv!(ctx.metrics.gv_current_pcie_link_generation, dl, device.current_pcie_link_gen()? as f64);
            set_gv!(ctx.metrics.gv_max_pcie_link_width, dl, device.max_pcie_link_width()? as f64);
            set_gv!(ctx.metrics.gv_max_pcie_link_generation, dl, device.max_pcie_link_gen()? as f64);
            set_gv!(ctx.metrics.gv_display_active, dl, if device.is_display_active()? { 1 } else { 0 });
            set_gv!(ctx.metrics.gv_display_mode, dl, if device.is_display_connected()? { 1 } else { 0 });

            match device.utilization_rates() {
                Ok(util) => {
                    set_gv!(ctx.metrics.gv_utilization_gpu, dl, util.gpu as f64);
                    set_gv!(ctx.metrics.gv_utilization_memory, dl, util.memory as f64);
                }
                Err(e) => warn!("error collecting utilization rates: {:?}", e),
            }

            match device.encoder_stats() {
                Ok(encoder_stats) => {
                    set_gv!(ctx.metrics.gv_encoder_capacity_h264, dl, device.encoder_capacity(EncoderType::H264)?);
                    set_gv!(ctx.metrics.gv_encoder_capacity_hevc, dl, device.encoder_capacity(EncoderType::HEVC)?);
                    set_gv!(ctx.metrics.gv_encoder_stats_sessions_count, dl, encoder_stats.session_count as f64);
                    set_gv!(ctx.metrics.gv_encoder_stats_average_fps, dl, encoder_stats.average_fps as f64);
                    set_gv!(ctx.metrics.gv_encoder_stats_average_latency, dl, encoder_stats.average_latency as f64);
                }
                Err(e) => warn!("error collecting encoder stats: {:?}", e),
            }

            match device.fbc_stats() {
                Ok(fbc_stats) => {
                    set_gv!(ctx.metrics.gv_fbc_stats_sessions_count, dl, fbc_stats.sessions_count as f64);
                    set_gv!(ctx.metrics.gv_fbc_stats_average_fps, dl, fbc_stats.average_fps as f64);
                    set_gv!(ctx.metrics.gv_fbc_stats_average_latency, dl, fbc_stats.average_latency as f64);
                }
                Err(e) => warn!("error collecting framebuffer capture stats: {:?}", e),
            }

            match device.memory_info() {
                Ok(mem) => {
                    ctx.metrics.gv_memory_info.with_label_values(&[dev_idx_str, dev_uuid, "free"]).set(mem.free as f64);
                    ctx.metrics.gv_memory_info.with_label_values(&[dev_idx_str, dev_uuid, "total"]).set(mem.total as f64);
                    ctx.metrics.gv_memory_info.with_label_values(&[dev_idx_str, dev_uuid, "used"]).set(mem.used as f64);
                }
                Err(e) => warn!("error fetching current memory info: {:?}", e),
            };
        });

        timed!("clocks", {
            /*
             * Only the "current" clock series seems to pull on @oko's RTX 3000 series card
             */
            for cid in &[ClockId::Current, ClockId::TargetAppClock, ClockId::DefaultAppClock, ClockId::CustomerMaxBoost] {
                let cid_str = clock_id_str(cid.clone());
                for ctype in &[Clock::Graphics, Clock::Memory, Clock::SM, Clock::Video] {
                    let ctype_str = clock_type_str(ctype.clone());
                    let clock = device.clock(ctype.clone(), cid.clone());
                    if clock.is_ok() {
                        if cfg!(debug_assertions) {
                            trace!("got metrics for clock ID {:?} and type {:?}", cid, ctype);
                        }
                        set_gv!(ctx.metrics.gv_clock, &[dev_idx_str, dev_uuid, cid_str, ctype_str], clock.unwrap() as f64);
                    }
                    let aclock = device.clock(ctype.clone(), cid.clone());
                    if aclock.is_ok() {
                        if cfg!(debug_assertions) {
                            trace!("got metrics for applications clock ID {:?} and type {:?}", cid, ctype);
                        }
                        set_gv!(ctx.metrics.gv_applications_clock, &[dev_idx_str, dev_uuid, cid_str, ctype_str], aclock.unwrap() as f64);
                    }
                }
            }
        });

        if ctx.opts.enable_throttle_reasons {
            timed!("throttle_reasons", {
                match device.current_throttle_reasons() {
                    Ok(throttle_reasons) => {
                        for reason in [
                            ThrottleReasons::GPU_IDLE,
                            ThrottleReasons::APPLICATIONS_CLOCKS_SETTING,
                            ThrottleReasons::SW_POWER_CAP,
                            ThrottleReasons::HW_SLOWDOWN,
                            ThrottleReasons::SYNC_BOOST,
                            ThrottleReasons::SW_THERMAL_SLOWDOWN,
                            ThrottleReasons::HW_THERMAL_SLOWDOWN,
                            ThrottleReasons::HW_POWER_BRAKE_SLOWDOWN,
                            ThrottleReasons::DISPLAY_CLOCK_SETTING,
                            ThrottleReasons::NONE,
                        ] {
                            ctx.metrics
                                .gv_current_clocks_throttle_reasons
                                .with_label_values(&[dev_idx_str, dev_uuid, throttle_reason_str(reason)])
                                .set(if throttle_reasons.contains(reason) { 1 } else { 0 } as f64);
                        }
                    }
                    Err(e) => warn!("error fetching current throttle reasons: {:?}", e),
                }
            });
        } else {
            warn!("skipping throttle reasons collection");
        }

        match device.is_ecc_enabled() {
            Ok(ecc_state) => {
                timed!("memory_errors", {
                    if ecc_state.currently_enabled {
                        debug!("ECC enabled, collecting memory error statistics");
                        for loc in [
                            MemoryLocation::Cbu,
                            MemoryLocation::Device,
                            MemoryLocation::L1Cache,
                            MemoryLocation::L2Cache,
                            MemoryLocation::RegisterFile,
                            MemoryLocation::Shared,
                            MemoryLocation::SRAM,
                            MemoryLocation::Texture,
                        ] {
                            for ecc in [EccCounter::Aggregate, EccCounter::Volatile] {
                                for err in [MemoryError::Corrected, MemoryError::Uncorrected] {
                                    match device.memory_error_counter(err.clone(), ecc.clone(), loc.clone()) {
                                        Ok(ct) => ctx
                                            .metrics
                                            .gv_memory_error_counters
                                            // &["device", "mem_error", "ecc_counter", "mem_location"]
                                            .with_label_values(&[dev_idx_str, dev_uuid, memory_error_type_str(&err), ecc_counter_type_str(&ecc), memory_location_str(&loc)])
                                            .set(ct as f64),
                                        Err(e) => {
                                            if cfg!(debug_assertions) {
                                                trace!("failed to collect {} {} {}: {:?}", memory_error_type_str(&err), ecc_counter_type_str(&ecc), memory_location_str(&loc), e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        warn!("ECC is not enabled, skipping memory error metrics");
                    }
                });
            }
            Err(e) => warn!("could not check ECC state, skipping memory error metrics: {:?}", e),
        }
    }
    debug!("NVML metrics gather took {}ms", now.elapsed().unwrap().as_millis());
    Ok(())
}
