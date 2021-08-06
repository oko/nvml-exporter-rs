extern crate nvml_wrapper as nvml;

use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, Version};
use nvml::enum_wrappers::device::TemperatureSensor;
use nvml::error::NvmlError;
use nvml::NVML;
use prometheus::{
    default_registry, Counter, Encoder, Gauge, GaugeVec, Opts, Registry, TextEncoder,
};
use prometheus::{register_gauge, register_gauge_vec};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;

#[tokio::main]
async fn main() {
    let nvml = Arc::new(NVML::init().unwrap());
    let m = Metrics::new().unwrap();
    let addr = SocketAddr::from(([127, 0, 0, 1], 9500));
    let arcm = Arc::new(m);

    let make_svc = make_service_fn(move |_: &AddrStream| {
        let msf_arcm = arcm.clone();
        let msf_nvml = nvml.clone();
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
                    } as core::result::Result<Response<Body>, &str>)
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    // Run this server for... forever!
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
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
                &["devices"]
            )?,
            gv_running_graphics_processes_count: register_gauge_vec!(
                "nvml_running_graphics_processes_count",
                "number of running graphics processes",
                &["devices"]
            )?,
        })
    }
}

macro_rules! set_gauge_value {
    ( $member:expr, $dev_label:expr, $val:expr ) => {
        $member.with_label_values(&[$dev_label]).set($val as f64)
    };
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
    }
    println!("gather: {}ms elapsed", now.elapsed().unwrap().as_millis());
    Ok(())
}
/*
fn gather2(nvml: NVML) -> Result<(), NvmlError> {
    let count = nvml.device_count()?;
    let device_count_gauge = prometheus::Gauge::new("nvml_device_count", "number of nvml devices")?;
}
 */
