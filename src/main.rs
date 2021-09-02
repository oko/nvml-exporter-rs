extern crate nvml_wrapper as nvml;

use clap::{App, Arg};
use futures::future::join_all;
use futures::SinkExt;
use hyper::server::conn::{AddrIncoming, AddrStream};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, Version};
use nvml::enum_wrappers::device::{Clock, ClockId, TemperatureSensor};
use nvml::error::NvmlError;
use nvml::NVML;
use prometheus::{
    default_registry, Counter, Encoder, Gauge, GaugeVec, Opts, Registry, TextEncoder,
};
use prometheus::{register_gauge, register_gauge_vec};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::signal;

async fn shutdown_signal(addr: &str) {
    // Wait for the CTRL+C signal
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    eprintln!("shutting down server on {}", addr);
}

#[tokio::main]
async fn main() {
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
                .default_values(&["[::]:9500", "0.0.0.0:9500"]),
        )
        .get_matches();
    let nvml = Arc::new(NVML::init().unwrap());
    let m = Metrics::new().unwrap();
    //let addr = SocketAddr::from_str(matches.value_of("listen").unwrap()).unwrap();
    let addr_strs = matches.values_of("listen").unwrap();
    let arcm = Arc::new(m);

    let servers = addr_strs
        .map(move |addr_str| {
            let bind_arcm = arcm.clone();
            let bind_nvml = nvml.clone();
            Server::bind(&SocketAddr::from_str(addr_str).unwrap())
                .serve(make_service_fn(move |_: &AddrStream| {
                    let msf_arcm = bind_arcm.clone();
                    let msf_nvml = bind_nvml.clone();
                    async move {
                        Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                            let sf_arcm = msf_arcm.clone();
                            let sf_nvml = msf_nvml.clone();
                            async move {
                                ({
                                    gather(sf_arcm, sf_nvml);
                                    // Gather the metrics.
                                    let mut buffer = vec![];
                                    let encoder = TextEncoder::new();
                                    let metric_families = default_registry().gather();
                                    encoder.encode(&metric_families, &mut buffer).unwrap();

                                    Ok(Response::new(Body::from(
                                        String::from_utf8(buffer).unwrap(),
                                    )))
                                }
                                    as core::result::Result<Response<Body>, &str>)
                            }
                        }))
                    }
                }))
                .with_graceful_shutdown(shutdown_signal(addr_str))
        })
        .collect::<Vec<_>>();

    let results = join_all(servers).await;
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
        _ => "UNKNOWN",
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

fn gather(m: Arc<Metrics>, nvml: Arc<NVML>) -> Result<(), NvmlError> {
    let now = SystemTime::now();
    println!("start");
    let count = nvml.device_count()?;
    m.g_device_count.set(count as f64);
    for device_index in 0..count {
        let device = nvml.device_by_index(device_index)?;
        let dev_idx_string = device_index.to_string();
        let dev_idx_str = dev_idx_string.as_str();
        m.gv_device_temp
            .with_label_values(&[dev_idx_str])
            .set(device.temperature(TemperatureSensor::Gpu)? as f64);

        set_gauge_value!(
            m.gv_device_temp,
            dev_idx_str,
            device.temperature(TemperatureSensor::Gpu)?
        );

        let fbc_stats = device.fbc_stats()?;
        m.gv_fbc_stats_sessions_count
            .with_label_values(&[dev_idx_str])
            .set(fbc_stats.sessions_count as f64);
        m.gv_fbc_stats_average_fps
            .with_label_values(&[dev_idx_str])
            .set(fbc_stats.average_fps as f64);
        m.gv_fbc_stats_average_latency
            .with_label_values(&[dev_idx_str])
            .set(fbc_stats.average_latency as f64);

        m.gv_device_power_usage
            .with_label_values(&[dev_idx_str])
            .set((device.power_usage()? as f64) / 1000.);

        m.gv_running_compute_processes_count
            .with_label_values(&[dev_idx_str])
            .set(device.running_compute_processes_count()? as f64);
        m.gv_running_graphics_processes_count
            .with_label_values(&[dev_idx_str])
            .set(device.running_graphics_processes_count()? as f64);

        m.gv_current_pcie_link_width
            .with_label_values(&[dev_idx_str])
            .set(device.current_pcie_link_width()? as f64);
        m.gv_current_pcie_link_generation
            .with_label_values(&[dev_idx_str])
            .set(device.current_pcie_link_gen()? as f64);
        m.gv_max_pcie_link_width
            .with_label_values(&[dev_idx_str])
            .set(device.max_pcie_link_width()? as f64);
        m.gv_max_pcie_link_generation
            .with_label_values(&[dev_idx_str])
            .set(device.max_pcie_link_gen()? as f64);

        let util = device.utilization_rates()?;
        m.gv_utilization_gpu
            .with_label_values(&[dev_idx_str])
            .set(util.gpu as f64);
        m.gv_utilization_memory
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
                    m.gv_clock
                        .with_label_values(&[dev_idx_str, cid_str, ctype_str])
                        .set(clock.unwrap() as f64);
                }
            }
        }
    }
    println!("nvml gather took {}ms", now.elapsed().unwrap().as_millis());
    Ok(())
}
