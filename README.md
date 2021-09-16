# `nvml-exporter-rs`: NVML Exporter for Prometheus

This is a Rust implementation of an NVML exporter for Prometheus.

## Prerequisites

* NVIDIA GPU drivers providing `nvml.dll` in `$env:PATH` (this should be the case by default using normal GeForce drivers)

## Building

Non-Windows-service binary:

```
cargo build
```

Windows service binary:

```shell
cargo build --bin nvml_exporter_svc --features=winsvc
```

## Running

Listen on wildcard v4/v6 (default):

```
./target/debug/nvml-exporter-rs.exe
```

Listen on a specific bind address:

```
./target/debug/nvml-exporter-rs.exe --listen 127.0.0.1:9500
```

## Exported Metrics

See the [NVML Device Queries](https://docs.nvidia.com/deploy/nvml-api/group__nvmlDeviceQueries.html) documentation potentially available metrics.

Currently implemented metrics are the fields of the `Metrics` struct in `main.rs`.

### Adding Metrics

New metrics may be added by:

1. Adding the field to the `Metrics` struct. `Gauge` should be used for global (system-wide) metrics, whereas `GaugeVec` should be used for metrics that are per-device.
2. Adding the field initialization to `Metrics::new()` with the appropriate macro.
3. Adding the collection implementation to `main::gather()`.

## Packaging

Build the Chocolatey package with:

```powershell
.\scripts\mkchoco.bat
```

Install the Chocolatey package you just built with:

```powershell
.\scripts\install.bat
```