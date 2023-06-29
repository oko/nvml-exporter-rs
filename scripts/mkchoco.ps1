$ErrorActionPreference = "Stop"
& cargo build --bin nvml_exporter_svc --release
Set-Location .\packaging\choco\nvml-exporter
& choco pack