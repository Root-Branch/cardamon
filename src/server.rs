mod errors;
mod routes;

use anyhow::Context;
use axum::response::{Html, IntoResponse, Response};
use axum::{http::header, routing::get, Router};
use colored::Colorize;
use http::{StatusCode, Uri};
use rust_embed::Embed;
use sea_orm::DatabaseConnection;

#[derive(Embed, Clone)]
#[folder = "src/public"]
struct Asset;

pub struct StaticFile<T>(pub T);

impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> Response {
        let path = self.0.into();

        match Asset::get(path.as_str()) {
            Some(content) => {
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        }
    }
}

// We use static route matchers ("/" and "/index.html") to serve our home
// page.
async fn index_handler() -> impl IntoResponse {
    static_handler("/index.html".parse::<Uri>().unwrap()).await
}

// We use a wildcard matcher ("/dist/*file") to match against everything
// within our defined assets directory. This is the directory on our Asset
// struct below, where folder = "examples/public/".
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.starts_with("dist/") {
        path = path.replace("dist/", "");
    }

    StaticFile(path)
}

// Finally, we use a fallback route for anything that didn't match.
async fn not_found() -> Html<&'static str> {
    Html("<h1>404</h1><p>Not Found</p>")
}

// Keep seperated for integraion tests
async fn create_app(db: &DatabaseConnection) -> Router {
    // Middleware later
    /*
    let protected = Router::new()
    .route("/user", get(routes::user::get_user))
    .layer(middleware::from_fn_with_state(pool.clone(), api_key_auth));
    */
    Router::new()
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        .route("/api/scenarios", get(routes::get_scenarios))
        .route("/api/runs/:scenario_name", get(routes::get_runs))
        .route("/*file", get(static_handler))
        .fallback_service(get(not_found))
        .with_state(db.clone())

    // let serve_assets = ServeEmbed::<Assets>::new();
    // Router::new().nest_service("/", serve_assets)
    // .layer(
    //     CorsLayer::new()
    //         .allow_methods([Method::GET, Method::POST])
    //         .allow_origin(Any),
    // )
}

pub async fn start(port: u32, db: &DatabaseConnection) -> anyhow::Result<()> {
    let app = create_app(db).await;

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    println!("\n{}", " Cardamon UI ".reversed().green());
    println!("> Server started: visit http://localhost:{}", port);
    axum::serve(listener, app).await.context("Error serving UI")
}
