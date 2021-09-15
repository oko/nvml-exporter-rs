extern crate nvml_exporter;
extern crate windows_service;

use log::{error, trace};
use std::{ffi::OsString, sync::mpsc, time::Duration};
use tokio::runtime::Runtime;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher, Result,
};

const SERVICE_NAME: &str = "nvml_exporter";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

pub fn run() -> Result<()> {
    // Register generated `ffi_service_main` with the system and start the service, blocking
    // this thread until the service is stopped.
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

define_windows_service!(ffi_service_main, my_service_main);

pub fn my_service_main(_arguments: Vec<OsString>) {
    if let Err(_e) = run_service() {
        error!("failed to run service");
    }
}

pub fn run_service() -> Result<()> {
    // Create a channel to be able to poll a stop event from the service worker loop.
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    // Define system service event handler that will be receiving service events.
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            // Notifies a service to report its current status information to the service
            // control manager. Always return NoError even if not implemented.
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,

            // Handle stop
            ServiceControl::Stop => {
                shutdown_tx.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }

            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register system service event handler.
    // The returned status handle should be used to report service status changes to the system.
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    // Tell the system that service is running
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let (binds, senders) = nvml_exporter::server_setup(
            std::env::args_os()
                .into_iter()
                .map(|s| s.into_string().unwrap())
                .collect(),
        );

        let server = tokio::task::spawn(async move {
            nvml_exporter::serve(binds).await;
        });

        let ctrl_c = tokio::task::spawn(async move {
            trace!("spawning ctrl-c handler");
            let _ = shutdown_rx.recv();
            senders.into_iter().for_each(|s| {
                let _ = s.send(());
            });
        });

        futures::future::join_all([server, ctrl_c]).await;
    });

    // Tell the system that service has stopped.
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

fn main() -> windows_service::Result<()> {
    run()
}
