use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    // In-memory store for graph data
    pub graph_data: Arc<Mutex<String>>,
    // In-memory store for POML data
    pub poml_data: Arc<Mutex<String>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            graph_data: Arc::new(Mutex::new(String::new())),
            poml_data: Arc::new(Mutex::new(String::new())),
        }
    }
}
