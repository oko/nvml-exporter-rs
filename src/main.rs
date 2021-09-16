extern crate nvml_exporter as nvml_exporter;
extern crate nvml_wrapper as nvml;

use futures::future::join_all;
use log::trace;

async fn server(args: Vec<String>) {
    let (binds, senders, options) = nvml_exporter::server_setup(args);
    let server = tokio::spawn(async move {
        nvml_exporter::serve(binds, options).await;
    });

    let ctrl_c = tokio::spawn(async move {
        trace!("spawning ctrl-c handler");
        // Wait for the CTRL+C signal
        tokio::signal::ctrl_c().await.expect("failed to install CTRL+C signal handler");
        trace!("received ctrl-c signal");
        senders.into_iter().for_each(|s| {
            let _ = s.send(());
        });
    });

    join_all([server, ctrl_c]).await;
}

#[tokio::main]
async fn main() {
    server(std::env::args_os().into_iter().map(|s| s.into_string().unwrap()).collect()).await;
}
