use anyhow::{Context, Result};
use opentelemetry::trace::{Span, SpanKind, Status, TraceContextExt, Tracer};
use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, TracerProvider};

use crate::config::Config;

/// Initialize OpenTelemetry tracing with OTLP/Zipkin exporter
pub fn init_tracing(config: &Config) -> Result<()> {
    // Only initialize if zipkin_endpoint is configured
    let endpoint = match &config.zipkin_endpoint {
        Some(ep) => ep,
        None => return Ok(()), // Tracing disabled
    };

    // Configure sampling based on sample_rate
    let sampler = if config.zipkin_sample_rate >= 1.0 {
        Sampler::AlwaysOn
    } else if config.zipkin_sample_rate <= 0.0 {
        Sampler::AlwaysOff
    } else {
        Sampler::TraceIdRatioBased(config.zipkin_sample_rate)
    };

    // Create OTLP exporter with Zipkin-compatible endpoint
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(format!("{}/api/v2/spans", endpoint))
        .build()
        .context("Failed to create OTLP exporter")?;

    // Create tracer provider with sampling configuration
    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_sampler(sampler)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(Resource::new(vec![
            KeyValue::new("service.name", config.zipkin_service_name.clone()),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ]))
        .build();

    // Set as global tracer provider
    global::set_tracer_provider(provider);

    Ok(())
}

/// Shutdown tracing and flush any pending spans
pub fn shutdown_tracing() {
    global::shutdown_tracer_provider();
}

/// Start a new span with the given name and kind
pub fn start_span(name: impl Into<String>, kind: SpanKind) -> opentelemetry::global::BoxedSpan {
    let tracer = global::tracer("ansible-piloteer");
    let mut span = tracer.start_with_context(name.into(), &opentelemetry::Context::current());
    span.set_attribute(KeyValue::new("span.kind", format!("{:?}", kind)));
    span
}

/// Record an error on a span
pub fn record_error_on_span(span: &mut opentelemetry::global::BoxedSpan, error: &str) {
    span.set_status(Status::error(error.to_string()));
    span.set_attribute(KeyValue::new("error", true));
    span.set_attribute(KeyValue::new("error.message", error.to_string()));
}

/// Add custom attributes to a span
pub fn add_span_attributes(span: &mut opentelemetry::global::BoxedSpan, attributes: Vec<KeyValue>) {
    for attr in attributes {
        span.set_attribute(attr);
    }
}

/// Record an error on the current span in context
pub fn record_error_on_current_span(error: &str) {
    use opentelemetry::trace::TraceContextExt;
    let cx = opentelemetry::Context::current();
    let span = cx.span();
    span.set_status(Status::error(error.to_string()));
    span.set_attribute(KeyValue::new("error", true));
    span.set_attribute(KeyValue::new("error.message", error.to_string()));
}

/// Add attributes to the current span in context
pub fn add_attributes_to_current_span(attributes: Vec<KeyValue>) {
    use opentelemetry::trace::TraceContextExt;
    let cx = opentelemetry::Context::current();
    let span = cx.span();
    for attr in attributes {
        span.set_attribute(attr);
    }
}

/// Execute a function within a span context
pub fn in_span<F, T>(name: impl Into<String>, _kind: SpanKind, f: F) -> T
where
    F: FnOnce() -> T,
{
    let tracer = global::tracer("ansible-piloteer");
    let span = tracer.start_with_context(name.into(), &opentelemetry::Context::current());
    let cx = opentelemetry::Context::current_with_span(span);
    let _guard = cx.attach();
    f()
}

/// Create a root span for playbook execution
pub fn create_root_span(
    name: impl Into<String>,
    attributes: Vec<KeyValue>,
) -> opentelemetry::global::BoxedSpan {
    let tracer = global::tracer("ansible-piloteer");
    let mut span = tracer.start(name.into());
    span.set_attribute(KeyValue::new("span.kind", "server"));
    for attr in attributes {
        span.set_attribute(attr);
    }
    span
}

/// Create a child span within the current context
pub fn create_child_span(
    name: impl Into<String>,
    attributes: Vec<KeyValue>,
) -> opentelemetry::global::BoxedSpan {
    let tracer = global::tracer("ansible-piloteer");
    let mut span = tracer.start_with_context(name.into(), &opentelemetry::Context::current());
    for attr in attributes {
        span.set_attribute(attr);
    }
    span
}

/// End a span with final attributes
pub fn end_span(mut span: opentelemetry::global::BoxedSpan, attributes: Vec<KeyValue>) {
    for attr in attributes {
        span.set_attribute(attr);
    }
    span.end();
}

/// Attach a span to the current context and return a guard
/// The guard must be kept alive for the span to remain active
pub fn attach_span(span: opentelemetry::global::BoxedSpan) -> opentelemetry::ContextGuard {
    let cx = opentelemetry::Context::current_with_span(span);
    cx.attach()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_disabled_when_no_endpoint() {
        let config = Config {
            openai_api_key: None,
            socket_path: "/tmp/test.sock".to_string(),
            model: "gpt-4".to_string(),
            api_base: "https://api.openai.com/v1".to_string(),
            log_level: "info".to_string(),
            auth_token: None,
            bind_addr: None,
            secret_token: None,
            quota_limit_tokens: None,
            quota_limit_usd: None,
            google_client_id: None,
            google_client_secret: None,
            zipkin_endpoint: None, // No endpoint = tracing disabled
            zipkin_service_name: "test".to_string(),
            zipkin_sample_rate: 1.0,
            filters: None, // [NEW]
        };

        // Should succeed without initializing tracing
        assert!(init_tracing(&config).is_ok());
    }
}
