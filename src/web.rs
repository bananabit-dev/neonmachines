pub mod api;
use crate::web::api::handlers;

use color_eyre::Result;
use tokio::signal;
use tracing::info;
use warp::Filter;

use crate::state::AppState;

pub async fn start_web_server(
    addr: std::net::SocketAddr,
    app_state: AppState,
) -> Result<()> {
    info!("Starting web server on {}", addr);

    // CORS support
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);

    // API routes
    let graph_api = {
        let app_state = app_state.clone();
        warp::path("api")
            .and(warp::path("graph"))
            .and(warp::get())
            .and_then(move || handlers::placeholder_handler(app_state.clone()))
    };


    // Static files
    let static_files = warp::path("static")
        .and(warp::fs::dir("web/static"));

    // Root route - serve index.html
    let index = warp::path::end()
        .and(warp::fs::file("web/index.html"));

    let poml_api = {
        let app_state = app_state.clone();
        warp::path("api")
            .and(warp::path("poml"))
            .and(warp::get())
            .and_then(move || handlers::placeholder_handler(app_state.clone()))
    };

    // Combine all routes that can produce rejections

    let poml_validate_api = warp::path!("api" / "poml" / "validate")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(handlers::validate_poml_handler);

    let all_routes = api::routes(app_state.clone())
        .or(graph_api)
        .or(poml_api)
        .or(poml_validate_api)
        .or(static_files)
        .or(index)
        .or(warp::path("graph").and(warp::fs::file("web/graph-editor.html")))
        .or(warp::path("poml").and(warp::fs::file("web/poml-editor.html")));

    // Apply CORS and the rejection handler to all routes
    let routes = all_routes
        .with(cors)
        .recover(api::handle_rejection);

    // Start the server
    warp::serve(routes)
        .run(addr)
        .await;

    Ok(())
}

pub async fn shutdown_signal() {
    info!("Shutdown signal received");

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown complete");
}
