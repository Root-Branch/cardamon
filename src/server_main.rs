mod server;
use axum::routing::{get, post, Router};
use dotenv::dotenv;
use server::metric_routes::{fetch_within, persist_metrics, scenario_iteration_persist};
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool};
use std::fs::File;
use tracing::{info, subscriber::set_global_default, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::writer::MakeWriterExt, layer::SubscriberExt, EnvFilter, Registry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let subscriber = get_subscriber("cardamon".into(), "debug".into());
    init_subscriber(subscriber);
    let pool = create_db().await?;
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
    let ui_router = Router::new()
        .route("/api/runs", method_router)
        .route("/api/runs/:run_id", method_router)
        .route("/api/scenarios/:scenario_id", method_router)
        .route("/api/metrics", method_router)
        .route("/api/cpu-metrics", method_router);
    Router::new()
        .merge(ui_router)
        .route("/cpu_metrics", post(persist_metrics))
        .route("/cpu_metrics/:id", get(fetch_within))
        //.route("/cpu_metrics/:id", delete(delete_metrics)) removed for now
        .route("/scenario", post(scenario_iteration_persist))
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
async fn create_db() -> anyhow::Result<SqlitePool> {
    let db_url = "sqlite://cardamon.db";
    if !sqlx::Sqlite::database_exists(db_url).await? {
        sqlx::Sqlite::create_database(db_url).await?;
    }

    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename("cardamon.db")
                .pragma("journal_mode", "DELETE"), // Disable WAL mode
        )
        // .connect(db_url) with wal and shm
        .await?;

    sqlx::migrate!().run(&db).await?;

    Ok(db)
}
