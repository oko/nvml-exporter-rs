extern crate nvml_exporter as nvml_exporter;
extern crate nvml_wrapper as nvml;

use universal_service::universal_service_main;

fn main() -> anyhow::Result<()> {
    universal_service_main(
        "nvml-exporter".to_owned(),
        Box::new(|shutdown_rx: std::sync::mpsc::Receiver<()>, args: Vec<String>, _start_parameters: Option<Vec<String>>| -> anyhow::Result<()> {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
            let handle = rt.handle().clone();
            let (binds, senders, options) = nvml_exporter::server_setup(args);
            std::thread::spawn(move || {
                handle.block_on(async move {
                    let _ = shutdown_rx.recv();
                    senders.into_iter().for_each(|s| {
                        let _ = s.send(());
                    });
                });
            });
            rt.block_on(nvml_exporter::serve(binds, options));
            Ok(())
        }),
    )?;
    Ok(())
}
