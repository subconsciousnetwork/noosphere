use crate::{cli::Cli, invoke_cli, CliContext};
use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use tempfile::TempDir;
use tracing::{field, Level, Subscriber};
use tracing_subscriber::{prelude::*, Layer};

#[derive(Default)]
struct InfoCaptureVisitor {
    message: String,
}

impl tracing::field::Visit for InfoCaptureVisitor {
    fn record_debug(&mut self, field: &field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message.push_str(&format!("{:?}", value));
        }
    }
}

#[derive(Default)]
struct InfoCaptureLayer {
    lines: Arc<Mutex<Vec<String>>>,
}

impl InfoCaptureLayer {
    pub fn lines(&self) -> Arc<Mutex<Vec<String>>> {
        self.lines.clone()
    }
}

impl<S> Layer<S> for InfoCaptureLayer
where
    S: Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().level() == &Level::INFO {
            let mut visitor = InfoCaptureVisitor::default();

            event.record(&mut visitor);

            let mut lines = self.lines.lock().unwrap();
            lines.push(visitor.message);
        }
    }
}

/// Poll a [Future] to completion, capturing the log output from work performed
/// in the span of that work.
pub fn run_and_capture_info<F>(fut: F) -> Result<Vec<String>>
where
    F: std::future::Future<Output = Result<()>>,
{
    let lines = tokio::task::block_in_place(|| {
        let capture_layer = InfoCaptureLayer::default();
        let lines = capture_layer.lines();

        let subscriber = tracing_subscriber::registry().with(capture_layer);
        tracing::subscriber::with_default(subscriber, || {
            tokio::runtime::Handle::current().block_on(async {
                fut.await?;
                Ok(()) as Result<_, anyhow::Error>
            })?;
            Ok(()) as Result<_, anyhow::Error>
        })?;

        let lines = lines.lock().unwrap().to_vec();

        Ok(lines) as Result<_, anyhow::Error>
    })?;

    Ok(lines)
}

/// A helper that simulates using the `orb` CLI from a command line. When
/// initialized, it produces a temporary directory for a sphere and for global
/// Noosphere configuration, and ensures that any command that expects these
/// things is configured to use them.
pub struct CliSimulator {
    current_working_directory: PathBuf,
    sphere_directory: TempDir,
    noosphere_directory: TempDir,
}

impl CliSimulator {
    /// Initialize a new [CliSimulator]
    pub fn new() -> Result<Self> {
        let sphere_directory = TempDir::new()?;

        Ok(CliSimulator {
            current_working_directory: sphere_directory.path().to_owned(),
            sphere_directory,
            noosphere_directory: TempDir::new()?,
        })
    }

    /// Logs a command to quickly move into the simulator's temporary
    /// directory and ensure its temporary credentials are available in your
    /// global Noosphere key storage location
    pub fn print_debug_shell_command(&self) {
        info!(
            "cd {} && cp {}/keys/* $HOME/.config/noosphere/keys/",
            self.sphere_directory().display(),
            self.noosphere_directory().display()
        );
    }

    /// The temporary root path for the [CliSimulator]'s sphere directory
    pub fn sphere_directory(&self) -> &Path {
        self.sphere_directory.path()
    }

    /// The temporary root path for the [CliSimulator]'s global Noosphere
    /// directory
    pub fn noosphere_directory(&self) -> &Path {
        self.noosphere_directory.path()
    }

    /// Change the current working directory used by this [CliSimulator] when
    /// simulating `orb` commands
    pub fn cd(&mut self, path: &Path) {
        self.current_working_directory = path.to_owned();
    }

    /// Change the current working directory to the temporary root sphere
    /// directory in use by this [CliSimulator]
    pub fn cd_to_sphere_directory(&mut self) {
        self.current_working_directory = self.sphere_directory().to_owned();
    }

    /// Run an `orb` command, capturing and returning its logged output
    pub async fn orb_with_output(&self, command: &[&str]) -> Result<Vec<String>> {
        Ok(self
            .run_orb_command(command, true)
            .await?
            .unwrap_or_default())
    }

    /// Run an `orb` command
    pub async fn orb(&self, command: &[&str]) -> Result<()> {
        self.run_orb_command(command, false).await?;
        Ok(())
    }

    fn parse_orb_command(&self, command: &[&str]) -> Result<Cli> {
        Ok(Cli::try_parse_from([&["orb"], command].concat())?)
    }

    async fn run_orb_command(
        &self,
        command: &[&str],
        capture_output: bool,
    ) -> Result<Option<Vec<String>>> {
        let cli = self.parse_orb_command(command)?;

        debug!(
            "In {}: orb {}",
            self.current_working_directory.display(),
            command.join(" ")
        );

        let context = CliContext {
            cwd: self.current_working_directory.clone(),
            global_config_dir: Some(self.noosphere_directory.path()),
        };
        let future = invoke_cli(cli, &context);
        if capture_output {
            let info = run_and_capture_info(future)?;
            Ok(Some(info))
        } else {
            future.await?;
            Ok(None)
        }
    }
}
