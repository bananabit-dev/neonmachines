use crate::state::AppState;
use warp::{Rejection, Reply};

pub async fn placeholder_handler(_app_state: AppState) -> Result<impl Reply, Rejection> {
    Ok(warp::reply::json(&"Placeholder API response".to_string()))
}

use serde::{Deserialize, Serialize};
use std::io::Write;
use tempfile::NamedTempFile;
use tokio::process::Command;

#[derive(Deserialize)]
pub struct ValidateRequest {
    poml: String,
}

#[derive(Serialize)]
pub struct ValidateResponse {
    status: String,
    message: String,
}

pub async fn validate_poml_handler(body: ValidateRequest) -> Result<impl Reply, Rejection> {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(body.poml.as_bytes()).unwrap();

    let output = Command::new("poml-cli")
        .arg("-f")
        .arg(file.path())
        .output()
        .await;

    let response = match output {
        Ok(output) => {
            if output.status.success() {
                ValidateResponse {
                    status: "ok".to_string(),
                    message: String::from_utf8_lossy(&output.stdout).to_string(),
                }
            } else {
                ValidateResponse {
                    status: "error".to_string(),
                    message: String::from_utf8_lossy(&output.stderr).to_string(),
                }
            }
        }
        Err(e) => ValidateResponse {
            status: "error".to_string(),
            message: e.to_string(),
        },
    };

    Ok(warp::reply::json(&response))
}
