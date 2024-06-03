use std::fs::File;

use axum::routing::{delete, get, post};
use axum::Router;
use dotenv::dotenv;
use server::routes::{delete_metrics, fetch_metrics, persist_metrics};
use sqlx::sqlite::SqlitePool;
use tracing::subscriber::set_global_default;
use tracing::{info, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};
mod server;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let subscriber = get_subscriber("cardamon".into(), "debug".into());
    init_subscriber(subscriber);
    let database_str = std::env::var("DATABASE_URL").expect("DATABASE_URL URL not set");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_lifetime(None)
        .idle_timeout(None)
        .max_connections(10)
        .connect(&database_str)
        .await?;
    let app = create_app(pool).await;
    let listener = tokio::net::TcpListener::bind(format!(
        "0.0.0.0:{}",
        std::env::var("SERVER_PORT").expect("Server port not set")
    ))
    .await
    .unwrap();
    info!("Starting cardamon server");
    axum::serve(listener, app).await.unwrap();
    Ok(())
}
// Keep seperated for integraion tests
async fn create_app(pool: SqlitePool) -> Router {
    // Middleware later
    /*
    let protected = Router::new()
    .route("/user", get(routes::user::get_user))
    .layer(middleware::from_fn_with_state(pool.clone(), api_key_auth));
    */
    Router::new()
        .route("/cpu_metrics", post(persist_metrics))
        .route("/cpu_metrics/:id", get(fetch_metrics))
        .route("/cpu_metrics/:id", delete(delete_metrics))
        .with_state(pool)
}
fn get_subscriber(name: String, env_filter: String) -> impl Subscriber + Sync + Send {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));

    let file_writer = File::create("debug.log").unwrap();
    let stdout_writer = std::io::stdout;
    let formatting_layer = BunyanFormattingLayer::new(name, file_writer.and(stdout_writer));

    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}
/*
 *
 * Print to *one* output ( e.g. std::io:stdout )
 * ( You need to pass in std::io::stdout as an argument then)
  fn get_subscriber<Sink>(
    name: String,
    env_filter: String,
    sink: Sink,
) -> impl Subscriber + Sync + Send
where
    Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name, sink);
    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}
 */

fn init_subscriber(subscriber: impl Subscriber + Sync + Send) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}
