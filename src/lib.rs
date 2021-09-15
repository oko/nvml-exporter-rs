extern crate nvml_wrapper as nvml;

use clap::{App, Arg};
use futures::channel::oneshot;
use std::str::FromStr;
use stderrlog;

use futures::channel::oneshot::{Receiver, Sender};
use futures::future::join_all;
use log::{debug, error, info, trace, warn};
use nvml::enum_wrappers::device::{Clock, ClockId, TemperatureSensor};
use nvml::error::NvmlError;
use nvml::NVML;
use prometheus::{default_registry, Encoder, Gauge, GaugeVec, TextEncoder};
use prometheus::{register_gauge, register_gauge_vec};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;
use warp::Filter;

pub fn server_setup(args: Vec<String>) -> (Vec<(SocketAddr, Receiver<()>)>, Vec<Sender<()>>) {
    let matches = App::new("nvml-exporter-rs")
        .version("0.0.1")
        .about("Prometheus exporter for NVIDIA GPU NVML metrics")
        .arg(
            Arg::new("listen")
                .short('l')
                .long("listen")
                .value_name("SOCKET_ADDRESS")
                .about("listen address")
                .multiple_occurrences(true)
                .takes_value(true)
                .default_values(&["[::]:9944", "0.0.0.0:9944"]),
        )
        .arg(
            Arg::new("verbosity")
                .short('v')
                .multiple_occurrences(true)
                .takes_value(false),
        )
        .get_matches_from(args);

    let verbosity = matches.occurrences_of("verbosity");
    stderrlog::new()
        .module(module_path!())
        .module("nvml_exporter")
        .verbosity(verbosity as usize)
        .show_module_names(true)
        .init()
        .unwrap();

    let addrs = matches
        .values_of("listen")
        .unwrap()
        .map(|val: &str| val.to_string())
        .collect::<Vec<_>>();

    let mut senders: Vec<Sender<()>> = vec![];

    let binds = addrs
        .iter()
        .map(|addr| {
            let (s, r) = oneshot::channel::<()>();
            senders.push(s);
            (SocketAddr::from_str(addr.clone().as_str()).unwrap(), r)
        })
        .collect::<Vec<_>>();

    (binds, senders)
}

struct Context {
    metrics: Metrics,
    nvml: NVML,
}

pub async fn serve(binds: Vec<(SocketAddr, Receiver<()>)>) {
    let ctx = Arc::new(Context {
        metrics: Metrics::new().unwrap(),
        nvml: NVML::init().unwrap(),
    });

    let mut joins: Vec<_> = vec![];
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
        joins.push(tokio::task::spawn(server));
    });

    let results = join_all(joins).await;
    for res in results.iter() {
        match res {
            Ok(_) => (),
            Err(e) => eprintln!("error during server shutdown: {}", e),
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
    gv_memory_info: GaugeVec,
}

impl Metrics {
    fn new() -> prometheus::Result<Metrics> {
        Ok(Metrics {
            g_device_count: register_gauge!("nvml_device_count", "number of nvml devices")?,
            gv_device_temp: register_gauge_vec!(
                "nvml_temperature",
                "temperature of nvml device",
                &["device"]
            )?,
            gv_device_power_usage: register_gauge_vec!(
                "nvml_power_usage",
                "power usage of nvml device",
                &["device"]
            )?,
            gv_fbc_stats_sessions_count: register_gauge_vec!(
                "nvml_fbc_stats_sessions_count",
                "session count for frame buffer capture sessions",
                &["device"]
            )?,
            gv_fbc_stats_average_fps: register_gauge_vec!(
                "nvml_fbc_stats_average_fps",
                "average fps for frame buffer capture sessions",
                &["device"]
            )?,
            gv_fbc_stats_average_latency: register_gauge_vec!(
                "nvml_fbc_stats_average_latency",
                "average latency for frame buffer capture sessions",
                &["device"]
            )?,
            gv_running_compute_processes_count: register_gauge_vec!(
                "nvml_running_compute_processes_count",
                "number of running compute processes",
                &["device"]
            )?,
            gv_running_graphics_processes_count: register_gauge_vec!(
                "nvml_running_graphics_processes_count",
                "number of running graphics processes",
                &["device"]
            )?,
            gv_current_pcie_link_width: register_gauge_vec!(
                "nvml_current_pcie_link_width",
                "current pcie link width",
                &["device"]
            )?,
            gv_current_pcie_link_generation: register_gauge_vec!(
                "nvml_current_pcie_link_generation",
                "current pcie link generation",
                &["device"]
            )?,
            gv_max_pcie_link_width: register_gauge_vec!(
                "nvml_max_pcie_link_width",
                "max pcie link width",
                &["device"]
            )?,
            gv_max_pcie_link_generation: register_gauge_vec!(
                "nvml_max_pcie_link_generation",
                "max pcie link generation",
                &["device"]
            )?,
            gv_utilization_gpu: register_gauge_vec!(
                "nvml_utilization_gpu",
                "GPU utilization",
                &["device"]
            )?,
            gv_utilization_memory: register_gauge_vec!(
                "nvml_utilization_memory",
                "memory utilization",
                &["device"]
            )?,
            gv_clock: register_gauge_vec!(
                "nvml_clock",
                "clock speed",
                &["device", "clock_id", "type"]
            )?,
            gv_memory_info: register_gauge_vec!(
                "nvml_memory_info",
                "memory information",
                &["device", "state"]
            )?,
        })
    }
}

macro_rules! set_gauge_value {
    ( $member:expr, $dev_label:expr, $val:expr ) => {
        $member.with_label_values(&[$dev_label]).set($val as f64)
    };
}

fn clock_id_str(cid: ClockId) -> &'static str {
    match cid {
        ClockId::Current => "current",
        ClockId::TargetAppClock => "app_clock_target",
        ClockId::DefaultAppClock => "app_clock_default",
        ClockId::CustomerMaxBoost => "customer_boost_max",
    }
}

fn clock_type_str(ctype: Clock) -> &'static str {
    match ctype {
        Clock::Graphics => "graphics",
        Clock::SM => "sm",
        Clock::Memory => "mem",
        Clock::Video => "video",
    }
}

fn gather(ctx: Arc<Context>) -> Result<(), NvmlError> {
    let now = SystemTime::now();
    debug!(
        "starting NVML gather at {}",
        chrono::Utc::now().format("%c").to_string()
    );
    let count = ctx.nvml.device_count()?;
    ctx.metrics.g_device_count.set(count as f64);
    for device_index in 0..count {
        let device = ctx.nvml.device_by_index(device_index)?;
        let dev_idx_string = device_index.to_string();
        let dev_idx_str = dev_idx_string.as_str();
        ctx.metrics
            .gv_device_temp
            .with_label_values(&[dev_idx_str])
            .set(device.temperature(TemperatureSensor::Gpu)? as f64);

        set_gauge_value!(
            ctx.metrics.gv_device_temp,
            dev_idx_str,
            device.temperature(TemperatureSensor::Gpu)?
        );

        let fbc_stats = device.fbc_stats()?;
        ctx.metrics
            .gv_fbc_stats_sessions_count
            .with_label_values(&[dev_idx_str])
            .set(fbc_stats.sessions_count as f64);
        ctx.metrics
            .gv_fbc_stats_average_fps
            .with_label_values(&[dev_idx_str])
            .set(fbc_stats.average_fps as f64);
        ctx.metrics
            .gv_fbc_stats_average_latency
            .with_label_values(&[dev_idx_str])
            .set(fbc_stats.average_latency as f64);

        ctx.metrics
            .gv_device_power_usage
            .with_label_values(&[dev_idx_str])
            .set((device.power_usage()? as f64) / 1000.);

        ctx.metrics
            .gv_running_compute_processes_count
            .with_label_values(&[dev_idx_str])
            .set(device.running_compute_processes_count()? as f64);
        ctx.metrics
            .gv_running_graphics_processes_count
            .with_label_values(&[dev_idx_str])
            .set(device.running_graphics_processes_count()? as f64);

        ctx.metrics
            .gv_current_pcie_link_width
            .with_label_values(&[dev_idx_str])
            .set(device.current_pcie_link_width()? as f64);
        ctx.metrics
            .gv_current_pcie_link_generation
            .with_label_values(&[dev_idx_str])
            .set(device.current_pcie_link_gen()? as f64);
        ctx.metrics
            .gv_max_pcie_link_width
            .with_label_values(&[dev_idx_str])
            .set(device.max_pcie_link_width()? as f64);
        ctx.metrics
            .gv_max_pcie_link_generation
            .with_label_values(&[dev_idx_str])
            .set(device.max_pcie_link_gen()? as f64);

        let util = device.utilization_rates()?;
        ctx.metrics
            .gv_utilization_gpu
            .with_label_values(&[dev_idx_str])
            .set(util.gpu as f64);
        ctx.metrics
            .gv_utilization_memory
            .with_label_values(&[dev_idx_str])
            .set(util.memory as f64);

        /*
         * Only the "current" clock series seems to pull on @oko's RTX 3000 series card
         */
        for cid in &[
            ClockId::Current,
            ClockId::TargetAppClock,
            ClockId::DefaultAppClock,
            ClockId::CustomerMaxBoost,
        ] {
            let cid_str = clock_id_str(cid.clone());
            for ctype in &[Clock::Graphics, Clock::Memory, Clock::SM, Clock::Video] {
                let ctype_str = clock_type_str(ctype.clone());
                let clock = device.clock(ctype.clone(), cid.clone());
                if clock.is_ok() {
                    if cfg!(debug_assertions) {
                        trace!("got metrics for clock ID {:?} and type {:?}", cid, ctype);
                    }
                    ctx.metrics
                        .gv_clock
                        .with_label_values(&[dev_idx_str, cid_str, ctype_str])
                        .set(clock.unwrap() as f64);
                }
            }
        }

        let mem = device.memory_info()?;
        ctx.metrics
            .gv_memory_info
            .with_label_values(&[dev_idx_str, "free"])
            .set(mem.free as f64);
        ctx.metrics
            .gv_memory_info
            .with_label_values(&[dev_idx_str, "total"])
            .set(mem.total as f64);
        ctx.metrics
            .gv_memory_info
            .with_label_values(&[dev_idx_str, "used"])
            .set(mem.used as f64);
    }
    debug!(
        "NVML metrics gather took {}ms",
        now.elapsed().unwrap().as_millis()
    );
    Ok(())
}
