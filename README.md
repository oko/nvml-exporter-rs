# `nvml-exporter-rs`: NVML Exporter for Prometheus

This is a Rust implementation of an NVML exporter for Prometheus.

## Prerequisites

* NVIDIA GPU drivers providing
    * Windows: `nvml.dll` in `$env:PATH` (this should be the case by default using normal GeForce drivers)
    * Linux: `libnvidia-ml.so` in `$LD_LIBRARY_PATH`
* **[Build only]:** Rust >1.53 installed

## Building

Non-Windows-service binary:

```
cargo build --bin nvml_exporter
```

Windows service binary:

```shell
cargo build --bin nvml_exporter_svc --features=winsvc
```

## Installing

### Linux

On Linux w/o dedicated packaging, copy the binary to `/usr/local/bin` and add a `systemd` unit file?:

```
[Unit]
Description=NVML Exporter
Wants=multi-user.target

[Service]
Type=simple
ExecStart=/usr/local/bin/nvml_exporter
```

### Windows

On Windows, you can use the `nvml-exporter` chocolatey package to install this. In order to preserve package `--param` flags from installation, first run:

```
choco feature enable --name=useRememberedArgumentsForUpgrades
```

Then install the package using:

```
choco install nvml-exporter
```

You can change the port used for listening (default=9996):

```
--params "'/ListenPort:12345'"
```

You can also enable collection of GPU throttling reasons (disabled by default):

```
--params "'/EnableThrottleReasons'"
```

ECC memory error collection is disabled unless ECC memory is available and currently in ECC mode. GeForce series GPUs do not have ECC memory.

If you need a custom package build for testing, see the "Packaging" section below.

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
4. New metrics should have their collection time impact measured with the `timed!` macro provided inline, and if costlier than a few milliseconds they should have an enable/disable mechanism flag added to the binary (see for example `--throttle-reasons`)

## Packaging

Build the Chocolatey package with:

```powershell
.\scripts\mkchoco.bat
```

Install the Chocolatey package you just built with:

```powershell
.\scripts\install.bat
```

## Release

Binary releases are currently created for Windows platforms **only**.

### Release Process

1. Push a commit bumping the version number in `packaging\choco\nvml-exporter\nvml-exporter.nuspec`
2. Create a new release on GitHub
3. Wait for the `release` GitHub Action to complete successfully
4. Open the `release` action summary and download the artifact zip file
5. Extract the `.nupkg` from the artifact zip and attach it to the release
6. Check https://community.chocolatey.org/account/Packages for the pending Chocolatey release
7. Make sure the new version passes moderation

### Release Mechanism

1. GitHub Action for the `release` event builds a `nupkg`
2. GitHub Action for the `release` event publishes the `nupkg` build to Chocolatey.org
3. The new version is held for moderation on Chocolatey.org
4. Any moderation issues on Chocolatey.org are resolved
5. Package is released for public visibility by Chocolatey.org package moderators