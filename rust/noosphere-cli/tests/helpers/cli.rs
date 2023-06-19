use tracing::*;

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use tempfile::TempDir;
use tracing::Level;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::Layer;

use clap::Parser;
use noosphere_cli::native::{cli::Cli, invoke_cli, workspace::Workspace};

#[derive(Default)]
struct InfoCaptureVisitor {
    message: String,
}

impl tracing::field::Visit for InfoCaptureVisitor {
    fn record_debug(&mut self, field: &field::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => {
                self.message.push_str(&format!("{:?}", value));
            }
            _ => (),
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
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        match event.metadata().level() {
            &Level::INFO => {
                let mut visitor = InfoCaptureVisitor::default();

                event.record(&mut visitor);

                let mut lines = self.lines.lock().unwrap();
                lines.push(visitor.message);
            }
            _ => (),
        };
    }
}

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

pub struct CliTestEnvironment {
    current_working_directory: PathBuf,
    sphere_directory: TempDir,
    noosphere_directory: TempDir,
}

impl CliTestEnvironment {
    pub fn new() -> Result<Self> {
        let sphere_directory = TempDir::new()?;

        Ok(CliTestEnvironment {
            current_working_directory: sphere_directory.path().to_owned(),
            sphere_directory,
            noosphere_directory: TempDir::new()?,
        })
    }

    pub fn sphere_directory(&self) -> &Path {
        self.sphere_directory.path()
    }

    pub fn noosphere_directory(&self) -> &Path {
        self.noosphere_directory.path()
    }

    pub fn cd(&mut self, path: &Path) {
        self.current_working_directory = path.to_owned();
    }

    pub fn cd_to_sphere_directory(&mut self) {
        self.current_working_directory = self.sphere_directory().to_owned();
    }

    pub async fn orb_with_output(&self, command: &[&str]) -> Result<Vec<String>> {
        Ok(self
            .run_orb_command(command, true)
            .await?
            .unwrap_or_default())
    }

    pub async fn orb(&self, command: &[&str]) -> Result<()> {
        self.run_orb_command(command, false).await?;
        Ok(())
    }

    fn parse_orb_command(&self, command: &[&str]) -> Result<Cli> {
        Ok(Cli::try_parse_from(&[&["orb"], command].concat())?)
    }

    async fn run_orb_command(
        &self,
        command: &[&str],
        capture_output: bool,
    ) -> Result<Option<Vec<String>>> {
        let cli = self.parse_orb_command(command)?;

        let workspace = Workspace::new(
            &self.current_working_directory,
            Some(self.noosphere_directory.path()),
        )?;

        debug!(
            "In {}: orb {}",
            self.current_working_directory.display(),
            command.join(" ")
        );

        if capture_output {
            let info = run_and_capture_info(invoke_cli(cli, workspace))?;
            Ok(Some(info))
        } else {
            invoke_cli(cli, workspace).await?;
            Ok(None)
        }
    }
}
