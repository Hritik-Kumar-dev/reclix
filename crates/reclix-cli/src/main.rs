//! reclix — command-line interface.
//!
//! Sends commands to `reclix-service` via D-Bus and prints status.
//!
//! Usage:
//!   reclix start
//!   reclix pause
//!   reclix resume
//!   reclix finish
//!   reclix status
//!   reclix mic         (toggle)
//!   reclix camera      (toggle — V0.3+)
//!   reclix focus       (toggle — V0.4+)
//!
//! All sub-commands except `status` send a [`ServiceCommand`] to the service
//! over D-Bus and print the resulting [`ServiceStatus`].

mod client;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};

use reclix_core::dbus_types::ServiceCommand;

#[derive(Parser)]
#[command(
    name        = "reclix",
    about       = "Reclix screen recorder CLI",
    version     = env!("CARGO_PKG_VERSION"),
    long_about  = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Start a new recording session.
    Start,
    /// Pause the current recording.
    Pause,
    /// Resume the current recording.
    Resume,
    /// Toggle pause / resume (alias).
    #[command(name = "toggle")]
    Toggle,
    /// Finish recording and export to MP4.
    Finish,
    /// Print current recording status.
    Status,
    /// Toggle the microphone on/off.
    Mic,
    /// Toggle the camera overlay (V0.3+).
    Camera,
    /// Toggle focus mode (V0.4+).
    Focus,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_env("RECLIX_LOG")
                .unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .without_time()
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Cmd::Status => {
            let status = client::get_status().await
                .context("could not reach reclix-service — is it running?")?;
            print_status(&status);
        }
        Cmd::Start   => send_and_print(ServiceCommand::Start).await?,
        Cmd::Pause |
        Cmd::Resume |
        Cmd::Toggle  => send_and_print(ServiceCommand::PauseResume).await?,
        Cmd::Finish  => send_and_print(ServiceCommand::Finish).await?,
        Cmd::Mic     => send_and_print(ServiceCommand::ToggleMic).await?,
        Cmd::Camera  => {
            eprintln!("Camera overlay not yet available (V0.3+).");
            std::process::exit(1);
        }
        Cmd::Focus   => {
            eprintln!("Focus mode not yet available (V0.4+).");
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn send_and_print(cmd: ServiceCommand) -> anyhow::Result<()> {
    let status = client::send_command(cmd).await
        .context("could not reach reclix-service — is it running?")?;
    print_status(&status);
    Ok(())
}

fn print_status(status: &reclix_core::dbus_types::ServiceStatus) {
    let secs = status.recording_secs;
    let mm = secs / 60;
    let ss = secs % 60;
    let time = format!("{mm:02}:{ss:02}");

    println!("state      : {}", status.state);
    println!("timer      : {time}");
    println!("segment    : {}", status.segment_index.map(|i| i.to_string()).unwrap_or("-".into()));
    println!("mic        : {}", if status.mic_enabled { "on" } else { "off" });
    println!("camera     : {}", if status.camera_enabled { "on" } else { "off" });
    if let Some(ref f) = status.output_file {
        println!("output     : {f}");
    }
}
