use actix_web::{web, App, HttpRequest, HttpServer, Responder};
use clap::clap_app;
use tracing::{debug, instrument};
use tracing_actix_web::TracingLogger;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

#[actix_web::main]
async fn main() {
    let matches = clap_app!(app =>
        (name: "simple-rust")
        (version: "1.0")
        (author: "Pierce Bartine (pbar)")
        (about: "A simple program to do simple things")
        (@setting SubcommandRequiredElseHelp)
        (@subcommand hello =>
            (about: "Says hello to the world")
        )
        (@subcommand webserver =>
            (about: "Starts an HTTP web server")
        )
    )
    .get_matches();

    match matches.subcommand_name() {
        Some("hello") => println!("Hello World!"),
        Some("webserver") => webserver().await.unwrap(),
        _ => panic!("impossible!"),
    }
}

async fn webserver() -> std::io::Result<()> {
    // Start an (optional) otel prometheus metrics pipeline
    let metrics_exporter = opentelemetry_prometheus::exporter().init();
    let request_metrics = actix_web_opentelemetry::RequestMetrics::new(
        opentelemetry::global::meter("actix_http_tracing"),
        Some(|req: &actix_web::dev::ServiceRequest| {
            req.path() == "/metrics" && req.method() == actix_web::http::Method::GET
        }),
        Some(metrics_exporter),
    );

    LogTracer::init().expect("Unable to setup log tracer!");
    let app_name = concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION")).to_string();
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let bunyan_formatting_layer = BunyanFormattingLayer::new(app_name, non_blocking_writer);
    let subscriber = Registry::default()
        .with(EnvFilter::new("DEBUG"))
        .with(JsonStorageLayer)
        .with(bunyan_formatting_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    // why is move used here?
    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger)
            .wrap(request_metrics.clone())
            .route("/", web::get().to(greet))
            .route("/{name}", web::get().to(greet))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

#[instrument]
async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    debug!("Preparing to respond with 'Hello {}!'", &name);
    format!("Hello {}!", &name)
}
