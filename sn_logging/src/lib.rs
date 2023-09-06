// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub mod appender;
pub mod error;
#[cfg(feature = "process-metrics")]
pub mod metrics;

use self::error::{Error, Result};
use std::fmt;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_core::{Event, Level, Subscriber};
use tracing_subscriber::{
    filter::Targets,
    fmt as tracing_fmt,
    fmt::{
        format::Writer,
        time::{FormatTime, SystemTime},
        FmtContext, FormatEvent, FormatFields,
    },
    layer::Filter,
    prelude::*,
    registry::LookupSpan,
    Layer, Registry,
};

const MAX_LOG_SIZE: usize = 20 * 1024 * 1024;
const MAX_UNCOMPRESSED_LOG_FILES: usize = 100;
const MAX_LOG_FILES: usize = 1000;

#[derive(Debug, Clone)]
pub enum LogOutputDest {
    Stdout,
    Path(PathBuf),
}

#[derive(Debug, Clone)]
pub enum LogFormat {
    Default,
    Json,
}

pub fn parse_log_format(val: &str) -> Result<LogFormat> {
    match val {
        "default" => Ok(LogFormat::Default),
        "json" => Ok(LogFormat::Json),
        _ => Err(Error::LoggingConfigurationError(
            "The only valid values for this argument are \"default\" or \"json\"".to_string(),
        )),
    }
}

impl fmt::Display for LogOutputDest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LogOutputDest::Stdout => write!(f, "stdout"),
            LogOutputDest::Path(p) => write!(f, "{}", p.to_string_lossy()),
        }
    }
}

const ALL_SN_LOGS: &str = "all";

#[derive(Default, Debug)]
/// Tracing log formatter setup for easier span viewing
pub struct LogFormatter;

impl<S, N> FormatEvent<S, N> for LogFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        // Write level and target
        let level = *event.metadata().level();
        let module = event.metadata().module_path().unwrap_or("<unknown module>");
        let time = SystemTime;

        write!(writer, "[")?;
        time.format_time(&mut writer)?;
        write!(writer, " {level} {module}")?;
        ctx.visit_spans(|span| write!(writer, "/{}", span.name()))?;
        write!(writer, "] ")?;

        // Add the log message and any fields associated with the event
        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

/// The different Subscribers composed into a list of layers
#[derive(Default)]
pub struct TracingLayers {
    layers: Vec<Box<dyn Layer<Registry> + Send + Sync>>,
    guard: Option<WorkerGuard>,
}

impl TracingLayers {
    fn fmt_layer(
        &mut self,
        default_logging_targets: Vec<(String, Level)>,
        output_dest: LogOutputDest,
        format: LogFormat,
    ) -> Result<()> {
        let layer = match output_dest {
            LogOutputDest::Stdout => {
                println!("Logging to stdout");
                tracing_fmt::layer()
                    .with_ansi(false)
                    .with_target(false)
                    .event_format(LogFormatter)
                    .boxed()
            }
            LogOutputDest::Path(ref path) => {
                std::fs::create_dir_all(path)?;
                println!("Logging to directory: {path:?}");

                let (file_rotation, worker_guard) = appender::file_rotater(
                    path,
                    MAX_LOG_SIZE,
                    MAX_UNCOMPRESSED_LOG_FILES,
                    MAX_LOG_FILES,
                );
                self.guard = Some(worker_guard);

                match format {
                    LogFormat::Json => tracing_fmt::layer()
                        .json()
                        .flatten_event(true)
                        .with_writer(file_rotation)
                        .boxed(),
                    LogFormat::Default => tracing_fmt::layer()
                        .with_ansi(false)
                        .with_writer(file_rotation)
                        .event_format(LogFormatter)
                        .boxed(),
                }
            }
        };
        let targets = match std::env::var("SN_LOG") {
            Ok(sn_log_val) => {
                println!("Using SN_LOG={sn_log_val}");
                get_logging_targets(&sn_log_val)?
            }
            Err(_) => default_logging_targets,
        };

        let target_filters: Box<dyn Filter<Registry> + Send + Sync> =
            Box::new(Targets::new().with_targets(targets));
        let layer = layer.with_filter(target_filters);
        self.layers.push(Box::new(layer));
        Ok(())
    }

    #[cfg(feature = "otlp")]
    fn otlp_layer(&mut self, default_logging_targets: Vec<(String, Level)>) -> Result<()> {
        use opentelemetry::{
            sdk::{trace, Resource},
            KeyValue,
        };
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_semantic_conventions::resource::{SERVICE_INSTANCE_ID, SERVICE_NAME};
        use rand::{distributions::Alphanumeric, thread_rng, Rng};

        let service_name = std::env::var("OTLP_SERVICE_NAME").unwrap_or_else(|_| {
            let random_node_name: String = thread_rng()
                .sample_iter(&Alphanumeric)
                .take(10)
                .map(char::from)
                .collect();
            random_node_name
        });
        println!("The opentelemetry traces are logged under the name: {service_name}");

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(opentelemetry_otlp::new_exporter().tonic().with_env())
            .with_trace_config(trace::config().with_resource(Resource::new(vec![
                KeyValue::new(SERVICE_NAME, service_name),
                KeyValue::new(SERVICE_INSTANCE_ID, std::process::id().to_string()),
            ])))
            .install_batch(opentelemetry::runtime::Tokio)?;

        let targets = match std::env::var("SN_LOG_OTLP") {
            Ok(sn_log_val) => {
                println!("Using SN_LOG_OTLP={sn_log_val}");
                get_logging_targets(&sn_log_val)?
            }
            Err(_) => default_logging_targets,
        };

        let target_filters: Box<dyn Filter<Registry> + Send + Sync> =
            Box::new(Targets::new().with_targets(targets));
        let otlp_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(target_filters)
            .boxed();
        self.layers.push(otlp_layer);
        Ok(())
    }
}

/// Inits node logging, returning the global node guard if required.
/// This guard should be held for the life of the program.
///
/// Logging should be instantiated only once.
pub fn init_logging(
    default_logging_targets: Vec<(String, Level)>,
    output_dest: LogOutputDest,
    format: LogFormat,
) -> Result<Option<WorkerGuard>> {
    let mut layers = TracingLayers::default();

    #[cfg(not(feature = "otlp"))]
    layers.fmt_layer(default_logging_targets, output_dest, format)?;

    #[cfg(feature = "otlp")]
    {
        layers.fmt_layer(default_logging_targets.clone(), output_dest, format)?;

        match std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            Ok(_) => layers.otlp_layer(default_logging_targets)?,
            Err(_) => println!(
                "The OTLP feature is enabled but the OTEL_EXPORTER_OTLP_ENDPOINT variable is not \
                set, so traces will not be submitted."
            ),
        }
    }

    tracing_subscriber::registry().with(layers.layers).init();

    Ok(layers.guard)
}

/// Initialize logger for tests, this is run only once, even if called multiple times.
#[cfg(feature = "test-utils")]
static TEST_INIT_LOGGER: std::sync::Once = std::sync::Once::new();
#[cfg(feature = "test-utils")]
pub fn init_test_logger() {
    TEST_INIT_LOGGER.call_once(|| {
        tracing_subscriber::fmt::fmt()
            // NOTE: uncomment this line for pretty printed log output.
            //.pretty()
            .with_ansi(false)
            .with_target(false)
            .event_format(LogFormatter)
            .try_init()
            .unwrap_or_else(|_| println!("Error initializing logger"));
    });
}

fn get_logging_targets(logging_env_value: &str) -> Result<Vec<(String, Level)>> {
    let mut targets = Vec::new();
    let crates = logging_env_value.split(',');
    for c in crates {
        // TODO: are there other default short-circuits wanted?
        // Could we have a default set if NOT on a release commit?
        if c == ALL_SN_LOGS {
            // short-circuit to get all logs
            return Ok(vec![
                ("safenode".to_string(), Level::TRACE),
                ("safe".to_string(), Level::TRACE),
                ("sn_build_info".to_string(), Level::TRACE),
                ("sn_cli".to_string(), Level::TRACE),
                ("sn_client".to_string(), Level::TRACE),
                ("sn_logging".to_string(), Level::TRACE),
                ("sn_networking".to_string(), Level::TRACE),
                ("sn_node".to_string(), Level::TRACE),
                ("sn_peers_acquisition".to_string(), Level::TRACE),
                ("sn_protocol".to_string(), Level::TRACE),
                ("sn_registers".to_string(), Level::TRACE),
                ("sn_testnet".to_string(), Level::TRACE),
                ("sn_transfers".to_string(), Level::TRACE),
            ]);
        }

        let mut split = c.split('=');
        let crate_name = split.next().ok_or_else(|| {
            Error::LoggingConfigurationError(
                "Could not obtain crate name in logging string".to_string(),
            )
        })?;
        let log_level = split.next().unwrap_or("trace");
        targets.push((crate_name.to_string(), get_log_level_from_str(log_level)?));
    }
    Ok(targets)
}

fn get_log_level_from_str(log_level: &str) -> Result<Level> {
    match log_level.to_lowercase().as_str() {
        "info" => Ok(Level::INFO),
        "debug" => Ok(Level::DEBUG),
        "trace" => Ok(Level::TRACE),
        "warn" => Ok(Level::WARN),
        "error" => Ok(Level::WARN),
        _ => Err(Error::LoggingConfigurationError(format!(
            "Log level {log_level} is not supported"
        ))),
    }
}
