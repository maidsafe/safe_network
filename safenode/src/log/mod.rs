// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod appender;
mod error;

use self::error::Result;

use std::fs;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_core::{Event, Subscriber};
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
        let time = SystemTime::default();

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
    fn fmt_layer(&mut self, optional_log_dir: &Option<PathBuf>) {
        // Filter by log level of this crate only
        let target_filters: Box<dyn Filter<Registry> + Send + Sync> = Box::new(
            Targets::new().with_target(current_crate_str(), tracing::Level::TRACE), // enable below for libp2p logs
                                                                                    // .with_target("libp2p", tracing::Level::TRACE),
        );
        let fmt_layer = tracing_fmt::layer().with_ansi(false);

        if let Some(log_dir) = optional_log_dir {
            // first remove old logs
            if fs::remove_dir_all(log_dir).is_ok() {
                println!("Removed old logs from directory: {log_dir:?}");
            }

            println!("Starting logging to directory: {log_dir:?}");

            let logs_max_lines = 5000;
            let logs_uncompressed = 100;
            let logs_max_files = 1000;

            let (non_blocking, worker_guard) =
                appender::file_rotater(log_dir, logs_max_lines, logs_uncompressed, logs_max_files);
            self.guard = Some(worker_guard);

            let fmt_layer = fmt_layer.with_writer(non_blocking);

            let layer = fmt_layer
                .event_format(LogFormatter::default())
                .with_filter(target_filters)
                .boxed();
            self.layers.push(layer);
        } else {
            println!("Starting logging to stdout");

            let layer = fmt_layer
                .with_target(false)
                .event_format(LogFormatter::default())
                .with_filter(target_filters)
                .boxed();
            self.layers.push(layer);
        };
    }

    #[cfg(feature = "otlp")]
    fn otlp_layer(&mut self) -> Result<()> {
        use opentelemetry::{
            sdk::{trace, Resource},
            KeyValue,
        };
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_semantic_conventions::resource::{SERVICE_INSTANCE_ID, SERVICE_NAME};
        use rand::{distributions::Alphanumeric, thread_rng, Rng};
        use tracing_subscriber::filter::LevelFilter;
        use tracing_subscriber::EnvFilter;

        let service_name = std::env::var("OTLP_SERVICE_NAME").unwrap_or_else(|_| {
            let random_node_name: String = thread_rng()
                .sample_iter(&Alphanumeric)
                .take(10)
                .map(char::from)
                .collect();
            format!("{}_{}", current_crate_str(), random_node_name)
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

        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .with_env_var("RUST_LOG_OTLP")
            .from_env_lossy();

        let otlp_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(env_filter)
            .boxed();
        self.layers.push(otlp_layer);
        Ok(())
    }
}

/// Inits node logging, returning the global node guard if required.
/// This guard should be held for the life of the program.
///
/// Logging should be instantiated only once.
pub fn init_node_logging(log_dir: &Option<PathBuf>) -> Result<Option<WorkerGuard>> {
    let mut layers = TracingLayers::default();
    layers.fmt_layer(log_dir);

    #[cfg(feature = "otlp")]
    {
        match std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            Ok(_) => layers.otlp_layer()?,
            Err(_) => info!(
                "The OTLP feature is enabled but the OTEL_EXPORTER_OTLP_ENDPOINT variable is not \
                set, so traces will not be submitted."
            ),
        }
    }

    tracing_subscriber::registry().with(layers.layers).init();

    Ok(layers.guard)
}

/// Initialize logger for tests, this is run only once, even if called multiple times.
#[cfg(test)]
static TEST_INIT_LOGGER: std::sync::Once = std::sync::Once::new();
#[cfg(test)]
pub fn init_test_logger() {
    TEST_INIT_LOGGER.call_once(|| {
        tracing_subscriber::fmt::fmt()
            // NOTE: uncomment this line for pretty printed log output.
            //.pretty()
            .with_ansi(false)
            .with_target(false)
            .event_format(crate::log::LogFormatter::default())
            .try_init()
            .unwrap_or_else(|_| println!("Error initializing logger"));
    });
}

/// Get current root module name (e.g. "safenode")
fn current_crate_str() -> &'static str {
    // Grab root from module path ("safenode::log::etc" -> "safenode")
    let m = module_path!();
    &m[..m.find(':').unwrap_or(m.len())]
}
