mod server;

use axum::routing::{get, post, Router};
use cardamon::data_access::LocalDAOService;
use http::Method;
use server::{iteration_routes, metric_routes, run_routes, scenario_routes, ui_routes};
use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool};
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, subscriber::set_global_default, Subscriber};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subscriber("debug".into());
    init_subscriber(subscriber);

    let pool = create_db().await?;
    let dao_service = LocalDAOService::new(pool.clone());
    let app = create_app(dao_service).await;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:7001".to_string())
        .await
        .unwrap();

    info!("Starting cardamon server");
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

// Keep seperated for integraion tests
async fn create_app(dao_service: LocalDAOService) -> Router {
    // Middleware later
    /*
    let protected = Router::new()
    .route("/user", get(routes::user::get_user))
    .layer(middleware::from_fn_with_state(pool.clone(), api_key_auth));
    */
    let ui_router = Router::new()
        .route("/api/scenarios", get(ui_routes::get_scenarios))
        .route("/api/database_url", get(ui_routes::get_database_url))
        .route("/api/scenarios/:scenario_id", get(ui_routes::get_scenario))
        .with_state(dao_service.clone());

    let metrics_router = Router::new()
        .route("/api/metrics", post(metric_routes::persist_metrics))
        .route("/api/metrics/:id", get(metric_routes::fetch_within))
        .with_state(dao_service.clone());

    let iteration_router = Router::new()
        .route("/api/iterations", get(iteration_routes::fetch_runs_all))
        .route(
            "/api/iterations/in_range",
            get(iteration_routes::fetch_runs_in_range),
        )
        .route(
            "/api/iterations/last_n",
            get(iteration_routes::fetch_runs_last_n),
        )
        .route("/api/iteration", post(iteration_routes::persist))
        .with_state(dao_service.clone());

    let run_router = Router::new()
        .route("/api/run", post(run_routes::persist))
        .with_state(dao_service.clone());

    let scenario_router = Router::new()
        .route("/api/scenarios", get(scenario_routes::fetch_all))
        .route("/api/scenarios/in_run", get(scenario_routes::fetch_in_run))
        .route(
            "/api/scenarios/in_range",
            get(scenario_routes::fetch_in_range),
        )
        .route(
            "/api/scenarios/by_name/:name",
            get(scenario_routes::fetch_by_name),
        )
        .with_state(dao_service.clone());

    Router::new()
        .merge(ui_router)
        .merge(metrics_router)
        .merge(iteration_router)
        .merge(run_router)
        .merge(scenario_router)
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST])
                .allow_origin(Any),
        )
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
