//! Helpers to wrangle logging across Noosphere crates
//! NOTE: [initialize_tracing] should only ever be called in tests or binaries;
//! a library should only concern itself with instrumentation and logging.
use strum_macros::{Display, EnumString};

/// The crates that are considered when evaluating [NoosphereLog] and
/// [NoosphereLogLevel] as directive configuration.
pub static NOOSPHERE_LOG_LEVEL_CRATES: &[&str] = &[
    "noosphere",
    "noosphere_core",
    "noosphere_storage",
    "noosphere_common",
    "noosphere_sphere",
    "noosphere_into",
    "noosphere_gateway",
    "noosphere_collections",
    "noosphere_cli",
    "noosphere_car",
    "noosphere_api",
    "noosphere_ns",
    "noosphere_ucan",
    "noosphere_ucan_key_support",
    "orb",
    "orb_ns",
    "tower_http",
];

/// Helpful preset for various levels of log verbosity. Log output depends on
/// two dimensions of configuration:
///
///  1. Filter level, which controls the granularity of the logs
///  2. Format, which controls the amount and layout of additional context for
///     each log line
///
/// [NoosphereLog] offers preset values that enable configuring both dimensions
/// in one go. The verbosity of the preset ascends from the first to the last.
#[derive(Clone, Display, EnumString)]
pub enum NoosphereLog {
    /// Equivalent to [NoosphereLogFormat::Minimal] / [NoosphereLogLevel::Off]
    #[strum(serialize = "silent")]
    Silent,
    /// Equivalent to [NoosphereLogFormat::Minimal] / [NoosphereLogLevel::Info]
    #[strum(serialize = "basic")]
    Basic,
    /// Equivalent to [NoosphereLogFormat::Minimal] / [NoosphereLogLevel::Debug]
    #[strum(serialize = "chatty")]
    Chatty,
    /// Equivalent to [NoosphereLogFormat::Verbose] / [NoosphereLogLevel::Debug]
    #[strum(serialize = "informed")]
    Informed,
    /// Equivalent to [NoosphereLogFormat::Pretty] / [NoosphereLogLevel::Debug]
    #[strum(serialize = "academic")]
    Academic,
    /// Equivalent to [NoosphereLogFormat::Verbose] / [NoosphereLogLevel::Trace]
    #[strum(serialize = "tiresome")]
    Tiresome,
    /// Equivalent to [NoosphereLogFormat::Pretty] / [NoosphereLogLevel::Trace]
    #[strum(serialize = "deafening")]
    Deafening,
}

impl From<NoosphereLog> for NoosphereLogFormat {
    fn from(noosphere_log: NoosphereLog) -> Self {
        match noosphere_log {
            NoosphereLog::Silent | NoosphereLog::Basic | NoosphereLog::Chatty => {
                NoosphereLogFormat::Minimal
            }
            NoosphereLog::Informed | NoosphereLog::Tiresome => NoosphereLogFormat::Verbose,
            NoosphereLog::Academic | NoosphereLog::Deafening => NoosphereLogFormat::Pretty,
        }
    }
}

impl From<NoosphereLog> for NoosphereLogLevel {
    fn from(noosphere_log: NoosphereLog) -> Self {
        match noosphere_log {
            NoosphereLog::Silent => NoosphereLogLevel::Off,
            NoosphereLog::Basic => NoosphereLogLevel::Info,
            NoosphereLog::Chatty | NoosphereLog::Informed | NoosphereLog::Academic => {
                NoosphereLogLevel::Debug
            }
            NoosphereLog::Tiresome | NoosphereLog::Deafening => NoosphereLogLevel::Trace,
        }
    }
}

/// The format used to display logs. The amount of minutia and noise in the format
/// increases in the order of the variants from top to bottom.
#[derive(Default, Clone, Display, EnumString)]
pub enum NoosphereLogFormat {
    /// As the name implies, this is the most minimal format. `INFO` events only
    /// display the contents of the log line. Other events are prefixed with
    /// their event name.
    #[default]
    #[strum(serialize = "minimal")]
    Minimal,
    /// Verbose formatting that includes minutia such as timestamps and code
    /// targets
    #[strum(serialize = "verbose")]
    Verbose,
    /// Extremely verbose formatting; each log spans multiple lines with
    /// additional whitespace for layout and includes the source file and line
    /// where the log originated.
    #[strum(serialize = "pretty")]
    Pretty,
    /// Structured, json-oriented logs which are well suited for automated log aggregation.
    #[strum(serialize = "structured")]
    Structured,
}

/// The filter level for the Noosphere-centric crates listed in
/// [NOOSPHERE_LOG_LEVEL_CRATES]. These filter levels correspond 1:1 with those
/// described in
/// [`env-filter`](https://docs.rs/env_logger/0.10.0/env_logger/#enabling-logging)
#[derive(Default, Clone, Display, EnumString)]
pub enum NoosphereLogLevel {
    /// Equivalent to [tracing::Level::TRACE]
    #[strum(serialize = "trace")]
    Trace,
    /// Equivalent to [tracing::Level::DEBUG]
    #[strum(serialize = "debug")]
    Debug,
    /// Equivalent to [tracing::Level::INFO]
    #[default]
    #[strum(serialize = "info")]
    Info,
    /// Equivalent to [tracing::Level::WARN]
    #[strum(serialize = "warn")]
    Warn,
    /// Equivalent to [tracing::Level::ERROR]
    #[strum(serialize = "error")]
    Error,
    /// Disables logging entirely
    #[strum(serialize = "off")]
    Off,
}

#[cfg(not(target_arch = "wasm32"))]
impl From<NoosphereLogLevel> for Vec<tracing_subscriber::filter::Directive> {
    fn from(noosphere_log_level: NoosphereLogLevel) -> Self {
        let mut directives = vec![];

        let log_level = noosphere_log_level.to_string();

        for name in NOOSPHERE_LOG_LEVEL_CRATES {
            if let Ok(directive) = format!("{name}={log_level}").parse() {
                directives.push(directive);
            }
        }

        directives
    }
}

#[cfg(target_arch = "wasm32")]
mod inner {
    use super::NoosphereLog;
    use std::sync::Once;
    static INITIALIZE_TRACING: Once = Once::new();

    /// Initialize tracing-based logging throughout the Noosphere body of code,
    /// as well as dependencies that implement tracing-based logging.
    pub fn initialize_tracing(_noosphere_log: Option<NoosphereLog>) {
        INITIALIZE_TRACING.call_once(|| {
            console_error_panic_hook::set_once();
            tracing_wasm::set_as_global_default();
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod inner {
    use anyhow::Result;
    use std::{marker::PhantomData, sync::Once};
    use tracing::{Event, Subscriber};
    use tracing_subscriber::{
        filter::Directive,
        fmt::{format, FmtContext, FormatEvent, FormatFields, FormattedFields, Layer as FmtLayer},
        prelude::*,
        registry::LookupSpan,
        EnvFilter, Layer, Registry,
    };

    // Mainly we disable this for iOS because it causes XCode
    // output to be very noisy/difficult to read.
    #[cfg(target_os = "ios")]
    const USE_ANSI_COLORS: bool = false;
    #[cfg(not(target_os = "ios"))]
    const USE_ANSI_COLORS: bool = true;

    use super::{NoosphereLog, NoosphereLogFormat, NoosphereLogLevel};

    #[cfg(docs)]
    use super::NOOSPHERE_LOG_LEVEL_CRATES;

    static INITIALIZE_TRACING: Once = Once::new();

    /// Initialize tracing-based logging throughout the Noosphere body of code,
    /// as well as dependencies that implement tracing-based logging.
    ///
    /// Invoking this function causes logs to be rendered until the termination
    /// of the program. The default behavior is for logs to be rendered to
    /// stdout. If this function is never called, logs will never be rendered.
    /// Invoking this function more than once has no effect.
    ///
    /// The function accepts an optional [NoosphereLog] preset configuration
    /// that controls log filter level and display format.
    ///
    /// Although this function accepts an optional preset configuration, it is
    /// also sensitive to specific environment variables. The following
    /// environment variables may be used to configure log behavior:
    ///
    ///  - **`RUST_LOG`**: A comma-separated list of directives as described in
    ///    [`tracing-subscriber`][1] and [`env-logger`][2] documentation
    ///  - **`NOOSPHERE_LOG`**: An optional preset value interpretted as a
    ///    [NoosphereLog]
    ///  - **`NOOSPHERE_LOG_LEVEL`**: A specific filter level that, if set, is
    ///    assigned to all of the [NOOSPHERE_LOG_LEVEL_CRATES]
    ///  - **`NOOSPHERE_LOG_FORMAT`**: A specific format that, if set, is
    ///    interpretted as a [NoosphereLogFormat] and configured as the format
    ///    for log output
    ///
    /// The configuration semantics are intended to be backwards compatible with
    /// those defined by `tracing-subscriber` and `env-logger`. If only
    /// `RUST_LOG` is set, then that configuration is applied as normal. The
    /// [NoosphereLogFormat::Verbose] format is used in this case.
    ///
    /// If some preset is given in the function invocation, that preset is
    /// decomposed to determine how to further modify the log filter level and
    /// format. The directives will be added to the ones prescribed by
    /// `RUST_LOG`. If a `NOOSPHERE_LOG` environment variable is specified, it
    /// will take precedence over preset given as an argument.
    ///
    /// Finally, if a `NOOSPHERE_LOG_LEVEL` and/or `NOOSPHERE_LOG_FORMAT`
    /// environment variable is configured, they will take final precedence to
    /// determine the log filter level and format respectively.
    ///
    /// [1]: https://docs.rs/tracing-subscriber/0.3.17/tracing_subscriber/filter/struct.EnvFilter.html#directives
    /// [2]: https://docs.rs/env_logger/0.10.0/env_logger/#enabling-logging
    pub fn initialize_tracing(noosphere_log: Option<NoosphereLog>) {
        INITIALIZE_TRACING.call_once(|| {
            if let Err(error) = initialize_tracing_subscriber::<
                Option<Box<dyn Layer<Registry> + Send + Sync>>,
            >(noosphere_log, None)
            {
                println!("Failed to initialize tracing: {}", error);
            }
        });
    }

    /// Identical to [initialize_tracing], but provides the ability to add in
    /// your own [Layer] for tracing.
    pub fn initialize_tracing_with_layer<T>(noosphere_log: Option<NoosphereLog>, layer: T)
    where
        T: Layer<Registry> + Send + Sync + Sized,
    {
        INITIALIZE_TRACING.call_once(|| {
            if let Err(error) = initialize_tracing_subscriber(noosphere_log, layer) {
                println!("Failed to initialize tracing: {}", error);
            }
        });
    }

    fn initialize_tracing_subscriber<T>(
        noosphere_log: Option<NoosphereLog>,
        layer: T,
    ) -> anyhow::Result<()>
    where
        T: Layer<Registry> + Send + Sync + Sized,
    {
        let rust_log_env = std::env::var("RUST_LOG").ok();
        let noosphere_log_env = std::env::var("NOOSPHERE_LOG").ok();
        let noosphere_log_level_env = std::env::var("NOOSPHERE_LOG_LEVEL").ok();
        let noosphere_log_format_env = std::env::var("NOOSPHERE_LOG_FORMAT").ok();

        let noosphere_log = match noosphere_log_env {
            Some(value) => match value.parse() {
                Ok(noosphere_log) => Some(noosphere_log),
                _ => noosphere_log,
            },
            None => noosphere_log,
        };

        let (mut noosphere_log_level_default, mut noosphere_log_format_default) =
            if rust_log_env.is_some() {
                (None, NoosphereLogFormat::Verbose)
            } else {
                (Some(NoosphereLogLevel::Info), NoosphereLogFormat::Minimal)
            };

        if let Some(noosphere_log) = noosphere_log {
            noosphere_log_level_default = Some(noosphere_log.clone().into());
            noosphere_log_format_default = noosphere_log.into();
        }

        let noosphere_log_level = match noosphere_log_level_env {
            Some(noosphere_log_level_env) => match noosphere_log_level_env.parse() {
                Ok(noosphere_log_level) => Some(noosphere_log_level),
                _ => noosphere_log_level_default,
            },
            None => noosphere_log_level_default,
        };

        let noosphere_log_format = match noosphere_log_format_env {
            Some(noosphere_log_format_env) => match noosphere_log_format_env.parse() {
                Ok(noosphere_log_format) => noosphere_log_format,
                _ => noosphere_log_format_default,
            },
            None => noosphere_log_format_default,
        };

        let mut env_filter = EnvFilter::default();
        let mut rust_log_directives = rust_log_env
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse())
            .collect::<Result<Vec<Directive>, _>>()
            .unwrap_or_else(|_| Vec::new());

        let mut directives: Vec<Directive> = match noosphere_log_level {
            Some(noosphere_log_level) => noosphere_log_level.into(),
            None => Vec::new(),
        };

        directives.append(&mut rust_log_directives);

        for directive in directives {
            env_filter = env_filter.add_directive(directive)
        }

        let subscriber = layer
            .and_then(env_filter)
            .with_subscriber(tracing_subscriber::registry());

        match noosphere_log_format {
            NoosphereLogFormat::Minimal => {
                let subscriber = subscriber.with(
                    FmtLayer::default().event_format(NoosphereMinimalFormatter::new(
                        tracing_subscriber::fmt::format()
                            .without_time()
                            .with_target(false)
                            .with_ansi(USE_ANSI_COLORS),
                    )),
                );

                #[cfg(feature = "sentry")]
                let subscriber = subscriber.with(sentry_tracing::layer());

                subscriber.init();
            }
            NoosphereLogFormat::Verbose => {
                let subscriber =
                    subscriber.with(tracing_subscriber::fmt::layer().with_ansi(USE_ANSI_COLORS));

                #[cfg(feature = "sentry")]
                let subscriber = subscriber.with(sentry_tracing::layer());

                subscriber.init();
            }
            NoosphereLogFormat::Pretty => {
                let subscriber =
                    subscriber.with(FmtLayer::default().pretty().with_ansi(USE_ANSI_COLORS));

                #[cfg(feature = "sentry")]
                let subscriber = subscriber.with(sentry_tracing::layer());

                subscriber.init();
            }
            NoosphereLogFormat::Structured => {
                let subscriber =
                    subscriber.with(FmtLayer::default().json().with_ansi(USE_ANSI_COLORS));

                #[cfg(feature = "sentry")]
                let subscriber = subscriber.with(sentry_tracing::layer());

                subscriber.init();
            }
        };

        Ok(())
    }

    /// A formatter designed to make `INFO` events display as closely as
    /// possible to regular `println` output, while allowing arbitrary other
    /// formatting for all other event types
    struct NoosphereMinimalFormatter<F, S, N>(F, PhantomData<S>, PhantomData<N>)
    where
        F: FormatEvent<S, N>,
        S: Subscriber + for<'a> LookupSpan<'a>,
        N: for<'a> FormatFields<'a> + 'static;

    impl<F, S, N> NoosphereMinimalFormatter<F, S, N>
    where
        F: FormatEvent<S, N>,
        S: Subscriber + for<'a> LookupSpan<'a>,
        N: for<'a> FormatFields<'a> + 'static,
    {
        pub fn new(formatter: F) -> Self {
            Self(formatter, PhantomData, PhantomData)
        }
    }

    impl<F, S, N> FormatEvent<S, N> for NoosphereMinimalFormatter<F, S, N>
    where
        F: FormatEvent<S, N>,
        S: Subscriber + for<'a> LookupSpan<'a>,
        N: for<'a> FormatFields<'a> + 'static,
    {
        fn format_event(
            &self,
            ctx: &FmtContext<'_, S, N>,
            mut writer: format::Writer<'_>,
            event: &Event<'_>,
        ) -> std::fmt::Result {
            let metadata = event.metadata();

            match metadata.level().as_str() {
                "INFO" => (),
                _ => {
                    return self.0.format_event(ctx, writer, event);
                }
            };

            if let Some(scope) = ctx.event_scope() {
                for span in scope.from_root() {
                    write!(writer, "{}", span.name())?;
                    let ext = span.extensions();
                    let fields = &ext
                        .get::<FormattedFields<N>>()
                        .expect("will never be `None`");
                    if !fields.is_empty() {
                        write!(writer, "{{{}}}", fields)?;
                    }
                    write!(writer, ": ")?;
                }
            }

            ctx.field_format().format_fields(writer.by_ref(), event)?;

            writeln!(writer)
        }
    }
}

pub use inner::*;
