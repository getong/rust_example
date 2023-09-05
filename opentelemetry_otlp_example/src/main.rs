use opentelemetry_api::trace::Tracer;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    // use tonic as grpc layer here.
    // If you want to use grpcio. enable `grpc-sys` feature and use with_grpcio function here.
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .install_simple()?;

    tracer.in_span("doing_work", |_cx| {
        // Traced app logic here...
    });

    Ok(())
}
