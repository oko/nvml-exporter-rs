cargo build --bin nvml_exporter_svc --features=winsvc --release
cd .\packaging\choco\nvml-exporter
choco pack