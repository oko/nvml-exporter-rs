cargo build --bin nvml_exporter_svc --features=winsvc
cd .\packaging\choco\nvml-exporter
choco pack