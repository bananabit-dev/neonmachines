use thiserror::Error;

#[derive(Error, Debug)]
pub enum NeonmachinesError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Workflow error: {0}")]
    Workflow(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("File system error: {0}")]
    FileSystem(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Rate limiting error: {0}")]
    RateLimit(String),

    #[error("POML execution error: {0}")]
    PomlExecution(String),

    #[error("TUI error: {0}")]
    Tui(String),

    #[error("CLI argument error: {0}")]
    Cli(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Unexpected error: {0}")]
    Unexpected(String),
}

impl NeonmachinesError {
    pub fn config<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::Config(msg.into())
    }

    pub fn workflow<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::Workflow(msg.into())
    }

    pub fn agent<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::Agent(msg.into())
    }

    pub fn file_system<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::FileSystem(msg.into())
    }

    pub fn network<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::Network(msg.into())
    }

    pub fn rate_limit<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::RateLimit(msg.into())
    }

    pub fn poml_execution<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::PomlExecution(msg.into())
    }

    pub fn tui<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::Tui(msg.into())
    }

    pub fn cli<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::Cli(msg.into())
    }

    pub fn parse<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::Parse(msg.into())
    }

    pub fn unexpected<S: Into<String>>(msg: S) -> Self {
        NeonmachinesError::Unexpected(msg.into())
    }
}

/// Result type alias for Neonmachines operations
pub type NeonmachinesResult<T> = Result<T, NeonmachinesError>;

/// Retry configuration for transient failures
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 10000,
            backoff_factor: 2.0,
        }
    }
}

/// Error types that are safe to retry
#[derive(Debug, Clone)]
pub enum RetryableErrorType {
    NetworkError,
    TimeoutError,
    RateLimited,
    TemporaryFailure,
    ResourceExhausted,
}

/// Check if an error is retryable
pub fn is_retryable_error<E: std::fmt::Debug>(error: &E) -> Option<RetryableErrorType> {
    let error_str = format!("{:?}", error);
    
    if error_str.contains("network") || error_str.contains("connection") || error_str.contains("timeout") {
        Some(RetryableErrorType::NetworkError)
    } else if error_str.contains("rate limit ") || error_str.contains("429") {
        Some(RetryableErrorType::RateLimited)
    } else if error_str.contains("timeout") || error_str.contains("deadline") {
        Some(RetryableErrorType::TimeoutError)
    } else if error_str.contains("resource") || error_str.contains("exhausted") {
        Some(RetryableErrorType::ResourceExhausted)
    } else if error_str.contains("temporary") || error_str.contains("retry") || error_str.contains("unavailable") {
        Some(RetryableErrorType::TemporaryFailure)
    } else {
        None
    }
}

/// Retry operation with exponential backoff
pub async fn retry_with_backoff<T, F, E>(
    config: &RetryConfig,
    operation: F,
) -> Result<T, E>
where
    F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>,
    E: std::fmt::Debug + Clone,
{
    let mut delay_ms = config.base_delay_ms;
    let mut last_error: Option<E> = None;
    let mut last_retryable_type: Option<RetryableErrorType> = None;

    for attempt in 0..config.max_attempts {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    info!("Operation succeeded on attempt {} after previous failures ", attempt + 1);
                }
                return Ok(result);
            }
            Err(e) => {
                last_error = Some(e.clone());
                
                // Check if this error is retryable
                let retryable_type = is_retryable_error(&e);
                
                if let Some(retryable) = retryable_type {
                    last_retryable_type = Some(retryable);
                    warn!(
                        "Attempt {} failed with retryable error {:?}, retrying in {}ms. Error: {:?}",
                        attempt + 1,
                        retryable,
                        delay_ms,
                        e
                    );

                    if attempt < config.max_attempts - 1 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                        delay_ms = (delay_ms as f64 * config.backoff_factor).min(config.max_delay_ms as f64) as u64;
                    }
                } else {
                    debug!("Attempt {} failed with non-retryable error: {:?}", attempt + 1, e);
                    break;
                }
            }
        }
    }

    // If we get here, all attempts failed
    if let Some(retryable_type) = last_retryable_type {
        error!(
            "Operation failed after {} attempts. Last retryable error type: {:?}. Final error: {:?}",
            config.max_attempts,
            retryable_type,
            last_error
        );
    } else {
        error!(
            "Operation failed after {} attempts with non-retryable error: {:?}",
            config.max_attempts,
            last_error
        );
    }

    Err(last_error.unwrap())
}

/// Circuit breaker state
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub failure_count: u32,
    pub max_failures: u32,
    pub last_failure_time: Option<std::time::Instant>,
    pub timeout_duration: std::time::Duration,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        CircuitBreaker {
            failure_count: 0,
            max_failures: 5,
            last_failure_time: None,
            timeout_duration: std::time::Duration::from_secs(30),
        }
    }
}

impl CircuitBreaker {
    pub fn new(max_failures: u32, timeout_duration: std::time::Duration) -> Self {
        CircuitBreaker {
            failure_count: 0,
            max_failures,
            last_failure_time: None,
            timeout_duration,
        }
    }

    pub fn is_circuit_open(&self) -> bool {
        if self.failure_count >= self.max_failures {
            if let Some(last_failure) = self.last_failure_time {
                if last_failure.elapsed() < self.timeout_duration {
                    return true;
                }
            }
        }
        false
    }

    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.last_failure_time = None;
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(std::time::Instant::now());
    }

    pub fn should_allow_request(&mut self) -> bool {
        if self.is_circuit_open() {
            false
        } else {
            true
        }
    }
}

/// Retry operation with circuit breaker protection
pub async fn retry_with_circuit_breaker<T, F, E>(
    circuit_breaker: &mut CircuitBreaker,
    config: &RetryConfig,
    operation: F,
) -> Result<T, E>
where
    F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>,
    E: std::fmt::Debug + Clone,
{
    // Check if circuit breaker is open
    if !circuit_breaker.should_allow_request() {
        return Err(E::from(format!("Circuit breaker is open. Please wait {} seconds.", 
            circuit_breaker.timeout_duration.as_secs())));
    }

    let result = retry_with_backoff(config, operation).await;

    match &result {
        Ok(_) => {
            circuit_breaker.record_success();
            info!("Circuit breaker reset after successful operation");
        }
        Err(e) => {
            circuit_breaker.record_failure();
            warn!("Circuit breaker failure recorded. Current failure count: {}", circuit_breaker.failure_count);
        }
    }

    result
}

/// Wrapper for generating API responses with retry logic
pub async fn generate_with_retry(
    base_url: String,
    api_key: String,
    model: String,
    temperature: f32,
    messages: Vec<llmgraph::models::tools::Message>,
    tools: Option<Vec<llmgraph::models::tools::Tool>>,
    retry_config: Option<RetryConfig>,
    circuit_breaker: Option<&mut CircuitBreaker>,
) -> Result<serde_json::Value, NeonmachinesError> {
    let config = retry_config.unwrap_or_else(RetryConfig::default);
    
    // Create a simple retry wrapper for generate_full_response
    let operation = || {
        let base_url_clone = base_url.clone();
        let api_key_clone = api_key.clone();
        let model_clone = model.clone();
        let messages_clone = messages.clone();
        let tools_clone = tools.clone();
        
        Box::pin(async move {
            let result = llmgraph::generate::generate::generate_full_response(
                base_url_clone,
                api_key_clone,
                model_clone,
                temperature,
                messages_clone,
                tools_clone,
            )
            .await;
            
            match result {
                Ok(response) => {
                    // Convert LLMResponse to Value for easier handling
                    let response_json = serde_json::json!({
                        "success": true,
                        "response": response,
                        "model": model,
                        "temperature": temperature
                    });
                    Ok(response_json)
                }
                Err(e) => Err(NeonmachinesError::Unexpected(format!("API call failed: {}", e)))
            }
        })
    };

    // Apply retry logic with optional circuit breaker
    if let Some(mut cb) = circuit_breaker {
        match retry_with_circuit_breaker(&mut cb, &config, operation).await {
            Ok(result) => Ok(result),
            Err(e) => Err(e)
        }
    } else {
        match retry_with_backoff(&config, operation).await {
            Ok(result) => Ok(result),
            Err(e) => Err(e)
        }
    }
}
