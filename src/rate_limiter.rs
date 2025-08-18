use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct RateLimiter {
    limits: Arc<Mutex<HashMap<String, RateLimit>>>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct RateLimit {
    count: u32,
    reset_time: Instant,
    limit: u32,
}

impl RateLimiter {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub fn new_with_limit(_limit: u32) -> Self {
        Self {
            limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub async fn check_rate_limit(&self, key: &str, limit: u32, window: Duration) -> bool {
        let mut limits = self.limits.lock().await;
        
        let now = Instant::now();
        let entry = limits.entry(key.to_string()).or_insert(RateLimit {
            count: 0,
            reset_time: now + window,
            limit,
        });

        // Reset window if expired
        if now > entry.reset_time {
            entry.count = 0;
            entry.reset_time = now + window;
        }

        // Check if under limit
        if entry.count < entry.limit {
            entry.count += 1;
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub async fn get_remaining(&self, key: &str) -> u32 {
        let limits = self.limits.lock().await;
        if let Some(limit) = limits.get(key) {
            (limit.limit - limit.count).max(0)
        } else {
            0
        }
    }
}

// Rate limiter functionality - simplified version
// TODO: Implement proper tower-governor integration later
#[allow(dead_code)]
pub fn create_governor_layer(_rate_limit: u32) {
    // Placeholder implementation
    println!("Rate limiting not yet fully implemented");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_rate_limiting() {
        let limiter = RateLimiter::new_with_limit(5);
        
        // Should allow first 5 requests
        for i in 0..5 {
            assert!(limiter.check_rate_limit("test", 5, Duration::from_secs(1)).await);
        }
        
        // Should deny 6th request
        assert!(!limiter.check_rate_limit("test", 5, Duration::from_secs(1)).await);
        
        // Should allow after window resets
        sleep(Duration::from_millis(1100)).await;
        assert!(limiter.check_rate_limit("test", 5, Duration::from_secs(1)).await);
    }
}
