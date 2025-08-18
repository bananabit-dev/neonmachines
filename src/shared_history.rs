use llmgraph::models::tools::Message;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SharedHistory {
    inner: Arc<Mutex<Vec<Message>>>,
}

impl SharedHistory {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn append(&self, msg: Message) {
        if let Ok(mut history) = self.inner.lock() {
            history.push(msg);
        }
    }

    #[allow(dead_code)]
    pub fn get_last(&self, n: usize) -> Vec<Message> {
        if let Ok(history) = self.inner.lock() {
            let len = history.len();
            history[len.saturating_sub(n)..].to_vec()
        } else {
            Vec::new()
        }
    }

    #[allow(dead_code)]
    pub fn search(&self, query: &str) -> Vec<Message> {
        if let Ok(history) = self.inner.lock() {
            history
                .iter()
                .filter(|m| {
                    m.content
                        .as_ref()
                        .map(|c| c.to_lowercase().contains(&query.to_lowercase()))
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }
}