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
use std::path::PathBuf;
use std::{collections::BTreeMap, fmt};
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

// Everything is logged by default
const ALL_SN_LOGS: &str = "all";
// Trace at nodes, clients, debug at networking layer
const VERBOSE_SN_LOGS: &str = "v";

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
    pub layers: Vec<Box<dyn Layer<Registry> + Send + Sync>>,
    pub guard: Option<WorkerGuard>,
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

/// Inits node logging, returning the NonBlocking guard if present.
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

    if tracing_subscriber::registry()
        .with(layers.layers)
        .try_init()
        .is_err()
    {
        println!("Tried to initialize and set global default subscriber more than once");
    }

    Ok(layers.guard)
}

/// Initialize fmt_layers for testing purposes.
///
/// subscriber.set_default() should be used if under a single threaded tokio / single threaded non-tokio context.
/// Refer here for more details: <https://github.com/tokio-rs/tracing/discussions/1626>
///
/// subscriber.init() should be used under multi threaded tokio context. If you have 1+ multithreaded tokio tests under
/// the same integration test, this might result in loss of logs. Hence use .init() (instead of .try_init()) to panic
/// if called more than once.
pub fn get_test_layers(
    default_logging_targets: Vec<(String, Level)>,
    output_dest: LogOutputDest,
    format: LogFormat,
) -> Result<TracingLayers> {
    let mut layers = TracingLayers::default();
    layers.fmt_layer(default_logging_targets, output_dest, format)?;
    Ok(layers)
}

/// Parses the logging targets from the env variable (SN_LOG). The crates should be given as a CSV, for e.g.,
/// `export SN_LOG = libp2p=DEBUG, tokio=INFO, all, sn_client=ERROR`
/// If any custom keyword is encountered in the CSV, for e.g., VERBOSE_SN_LOGS ('all'), then they might override some
/// of the value that you might have provided, `sn_client=ERROR` in the above example will be ignored and
/// instead will be set to `TRACE` since `all` keyword is provided.
fn get_logging_targets(logging_env_value: &str) -> Result<Vec<(String, Level)>> {
    let mut targets = BTreeMap::new();
    let mut contains_keyword_all_sn_logs = false;
    let mut contains_keyword_verbose_sn_logs = false;

    for crate_log_level in logging_env_value.split(',') {
        // TODO: are there other default short-circuits wanted?
        // Could we have a default set if NOT on a release commit?
        if crate_log_level == ALL_SN_LOGS {
            contains_keyword_all_sn_logs = true;
            continue;
        } else if crate_log_level == VERBOSE_SN_LOGS {
            contains_keyword_verbose_sn_logs = true;
            continue;
        }

        let mut split = crate_log_level.split('=');
        let crate_name = split.next().ok_or_else(|| {
            Error::LoggingConfigurationError(
                "Could not obtain crate name in logging string".to_string(),
            )
        })?;
        let log_level = split.next().unwrap_or("trace");
        targets.insert(crate_name.to_string(), get_log_level_from_str(log_level)?);
    }

    // dealing with keywords
    let networking_log_level = if contains_keyword_all_sn_logs {
        ("sn_networking".to_string(), Level::TRACE)
    } else {
        ("sn_networking".to_string(), Level::DEBUG)
    };
    if contains_keyword_all_sn_logs || contains_keyword_verbose_sn_logs {
        // extend will overwrite values inside `targets`
        targets.extend(vec![
            networking_log_level,
            ("safenode".to_string(), Level::TRACE),
            ("safe".to_string(), Level::TRACE),
            ("sn_build_info".to_string(), Level::TRACE),
            ("sn_cli".to_string(), Level::TRACE),
            ("sn_client".to_string(), Level::TRACE),
            ("sn_logging".to_string(), Level::TRACE),
            ("sn_node".to_string(), Level::TRACE),
            ("sn_peers_acquisition".to_string(), Level::TRACE),
            ("sn_protocol".to_string(), Level::TRACE),
            ("sn_registers".to_string(), Level::TRACE),
            ("sn_testnet".to_string(), Level::TRACE),
            ("sn_transfers".to_string(), Level::TRACE),
        ]);
    }
    Ok(targets
        .into_iter()
        .map(|(crate_name, level)| (crate_name, level))
        .collect())
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
