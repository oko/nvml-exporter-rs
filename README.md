# `nvml-exporter-rs`: NVML Exporter for Prometheus

This is a Rust implementation of an NVML exporter for Prometheus.

## Prerequisites

* NVIDIA GPU drivers providing `nvml.dll` in `$env:PATH` (this should be the case by default using normal GeForce drivers)

## Building

```
cargo build
```

## Running

```
./target/debug/nvml-exporter-rs.exe
```
