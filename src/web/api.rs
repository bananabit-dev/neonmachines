pub mod handlers;
pub mod models;

use crate::state::AppState;
use warp::{Filter, Rejection, Reply};
use std::convert::Infallible;

pub fn routes(app_state: AppState) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("api")
        .and(warp::get())
        .and_then(move || handlers::placeholder_handler(app_state.clone()))
}

pub async fn handle_rejection(_err: Rejection) -> Result<impl Reply, Infallible> {
    // For now, just return a simple error message
    Ok(warp::reply::with_status(
        "Internal Server Error",
        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
    ))
}
