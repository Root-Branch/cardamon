mod server;
use axum::extract::FromRef;
use axum::routing::{get, post, Router};
use cardamon::data_access::LocalDAOService;
use http::Method;
use server::{
    metric_routes::{fetch_within, persist_metrics, scenario_iteration_persist},
    ui_routes::{get_database_url, get_scenario, get_scenarios},
};
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool};
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, subscriber::set_global_default, Subscriber};
use tracing_log::LogTracer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subscriber("debug".into());
    init_subscriber(subscriber);

    let pool = create_db().await?;
    let data_access_service = LocalDAOService::new(pool.clone());
    let app = create_app(pool, data_access_service).await;

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:7001"))
        .await
        .unwrap();

    info!("Starting cardamon server");
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
#[derive(Clone, FromRef)]
struct AppState {
    pool: SqlitePool,
    dao_service: LocalDAOService,
}
// Keep seperated for integraion tests
async fn create_app(pool: SqlitePool, dao_service: LocalDAOService) -> Router {
    // Middleware later
    /*
    let protected = Router::new()
    .route("/user", get(routes::user::get_user))
    .layer(middleware::from_fn_with_state(pool.clone(), api_key_auth));
    */
    let app_state = AppState { pool, dao_service };
    let ui_router = Router::new()
        .route("/api/scenarios", get(get_scenarios))
        .route("/api/database_url", get(get_database_url))
        .route("/api/scenarios/:scenario_id", get(get_scenario));

    Router::new()
        .merge(ui_router)
        .route("/cpu_metrics", post(persist_metrics))
        .route("/cpu_metrics/:id", get(fetch_within))
        //.route("/cpu_metrics/:id", delete(delete_metrics)) removed for now
        .route("/scenario", post(scenario_iteration_persist))
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST])
                .allow_origin(Any),
        )
        .with_state(app_state)
}

fn get_subscriber(env_filter: String) -> impl Subscriber + Sync + Send {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false) // Optionally disable printing the target
        .pretty()
        .finish()
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
