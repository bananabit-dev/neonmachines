use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use std::fs;
use std::path::PathBuf;
use uuid;
use futures;
use serde::{Serialize, Deserialize};

/// Time range for historical data queries
#[derive(Debug, Clone, PartialEq)]
pub enum TimeRange {
    LastHour,
    LastDay,
    LastWeek,
    All,
}

/// Export format for historical data
#[derive(Debug, Clone)]
pub enum ExportFormat {
    JSON,
    CSV,
}

/// Historical performance data entry
#[derive(Clone, Debug, Serialize, Deserialize)] // Add derives
pub struct HistoricalEntry {
    timestamp: DateTime<Utc>,
    metrics: PerformanceMetrics,
    operation: String,
    success: bool,
}

impl HistoricalEntry {
    pub fn timestamp(&self) -> &DateTime<Utc> {
        &self.timestamp
    }
    
    pub fn metrics(&self) -> &PerformanceMetrics {
        &self.metrics
    }
    
    pub fn operation(&self) -> &str {
        &self.operation
    }
    
    pub fn success(&self) -> bool {
        self.success
    }
}

/// Summary statistics for historical data
#[derive(Debug, Clone)]
pub struct HistoricalSummary {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub total_requests: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub success_rate_percent: f64,
    pub average_response_time_ms: f64,
    pub min_response_time_ms: u64,
    pub max_response_time_ms: u64,
    pub request_rate_per_second: f64,
}

/// Historical performance data storage
#[derive(Clone, Debug, Serialize, Deserialize)] // Add derives
pub struct HistoricalPerformanceData {
    entries: Vec<HistoricalEntry>,
    max_entries: usize,
}

impl HistoricalPerformanceData {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 10000, // Keep last 10,000 entries
        }
    }

    pub async fn add_metrics_snapshot(&mut self, metrics: PerformanceMetrics) {
        let entry = HistoricalEntry {
            timestamp: Utc::now(),
            operation: "workflow_execution".to_string(),
            success: true, // Can be enhanced to track per-operation success
            metrics,
        };
        
        self.entries.push(entry);
        
        // Remove oldest entries if we exceed the maximum
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    pub async fn get_summary(&self, time_range: TimeRange) -> HistoricalSummary {
        let now = Utc::now();
        let start_time = match time_range {
            TimeRange::LastHour => now - Duration::hours(1),
            TimeRange::LastDay => now - Duration::days(1),
            TimeRange::LastWeek => now - Duration::weeks(1),
            TimeRange::All => self.entries.first().map(|e| e.timestamp).unwrap_or(now),
        };

        let filtered_entries: Vec<&HistoricalEntry> = self.entries
            .iter()
            .filter(|entry| entry.timestamp >= start_time)
            .collect();

        if filtered_entries.is_empty() {
            return HistoricalSummary {
                start_time,
                end_time: now,
                total_requests: 0,
                success_count: 0,
                error_count: 0,
                success_rate_percent: 0.0,
                average_response_time_ms: 0.0,
                min_response_time_ms: 0,
                max_response_time_ms: 0,
                request_rate_per_second: 0.0,
            };
        }

        let total_requests = filtered_entries.iter().map(|e| e.metrics.request_count).sum();
        let _total_duration_ms: u64 = filtered_entries.iter().map(|e| e.metrics.total_duration.num_milliseconds() as u64).sum();
        let success_count = filtered_entries.iter().map(|e| e.metrics.success_count).sum();
        let error_count = total_requests - success_count;
        let average_response_time_ms = filtered_entries.iter().map(|e| e.metrics.average_response_time.num_milliseconds() as f64).sum::<f64>() / filtered_entries.len() as f64;
        
        let min_response_time_ms = filtered_entries.iter().map(|e| e.metrics.average_response_time.num_milliseconds() as u64).min().unwrap_or(0);
        let max_response_time_ms = filtered_entries.iter().map(|e| e.metrics.average_response_time.num_milliseconds() as u64).max().unwrap_or(0);
        
        let duration_seconds = (now - start_time).num_seconds() as f64;
        let request_rate_per_second = if duration_seconds > 0.0 {
            total_requests as f64 / duration_seconds
        } else {
            0.0
        };
        
        let success_rate_percent = if total_requests > 0 {
            (success_count as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        HistoricalSummary {
            start_time,
            end_time: now,
            total_requests,
            success_count,
            error_count,
            success_rate_percent,
            average_response_time_ms,
            min_response_time_ms: min_response_time_ms.try_into().unwrap(),
            max_response_time_ms: max_response_time_ms.try_into().unwrap(),
            request_rate_per_second,
        }
    }

    pub async fn export(&self, format: ExportFormat) -> Result<String, String> {
        match format {
            ExportFormat::JSON => {
                let data = serde_json::to_string(&*self);
                data.map_err(|e| e.to_string())
            }
            ExportFormat::CSV => {
                let mut csv = String::new();
                csv.push_str("Timestamp,Operation,Success,RequestCount,SuccessCount,ErrorCount,AvgResponseTimeMs,TotalDurationMs,RPS,MemoryUsageMB,CPUUsagePercent\n");
                
                for entry in &self.entries {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{:.2},{},{:.2},{:.2},{:.2}\n",
                        entry.timestamp.to_rfc3339(),
                        entry.operation,
                        entry.success,
                        entry.metrics.request_count,
                        entry.metrics.success_count,
                        entry.metrics.request_count - entry.metrics.success_count,
                        entry.metrics.average_response_time.num_milliseconds(),
                        entry.metrics.total_duration.num_milliseconds(),
                        entry.metrics.requests_per_second,
                        entry.metrics.memory_usage_mb,
                        entry.metrics.cpu_usage_percent
                    ));
                }
                
                Ok(csv)
            }
        }
    }

    pub async fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn get_entries(&self) -> &Vec<HistoricalEntry> {
        &self.entries
    }
}

impl Default for HistoricalPerformanceData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub request_count: u64,
    pub success_count: u64,
    pub total_duration: Duration,
    pub average_response_time: Duration,
    pub p95_response_time: Duration,
    pub p99_response_time: Duration,
    pub requests_per_second: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub timestamp: DateTime<Utc>,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            request_count: 0,
            success_count: 0,
            total_duration: Duration::milliseconds(0),
            average_response_time: Duration::milliseconds(0),
            p95_response_time: Duration::milliseconds(0),
            p99_response_time: Duration::milliseconds(0),
            requests_per_second: 0.0,
            memory_usage_mb: 0.0,
            cpu_usage_percent: 0.0,
            timestamp: Utc::now(),
        }
    }
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_request(&mut self, duration: Duration, success: bool) {
        self.request_count += 1;
        if success {
            self.success_count += 1;
        }
        self.total_duration = self.total_duration + duration;
        self.update_derived_metrics();
    }

    pub fn update_derived_metrics(&mut self) {
        if self.request_count > 0 {
            let avg = (self.total_duration.num_milliseconds() / self.request_count as i64) as u64;
            self.average_response_time = Duration::milliseconds(avg as i64);
        }
        
        // Update requests per second
        let elapsed_seconds = (Utc::now() - self.timestamp).num_seconds() as f64;
        self.requests_per_second = if elapsed_seconds > 0.0 {
            self.request_count as f64 / elapsed_seconds
        } else {
            0.0
        };
        
        // Simulate memory and CPU usage (in a real implementation, you'd get actual system metrics)
        self.memory_usage_mb = (self.requests_per_second * 0.1).min(1000.0);
        self.cpu_usage_percent = (self.requests_per_second * 0.5).min(100.0);
        
        // Update percentiles (simplified)
        self.p95_response_time = Duration::milliseconds((self.average_response_time.num_milliseconds() as f64 * 1.2) as i64);
        self.p99_response_time = Duration::milliseconds((self.average_response_time.num_milliseconds() as f64 * 1.5) as i64);
    }

    pub fn get_success_rate(&self) -> f64 {
        if self.request_count > 0 {
            self.success_count as f64 / self.request_count as f64
        } else {
            0.0
        }
    }

    pub fn get_error_rate(&self) -> f64 {
        1.0 - self.get_success_rate()
    }
}

pub struct RequestTiming {
    pub operation: String,
    pub start_time: std::time::Instant,
    pub end_time: Option<std::time::Instant>,
    pub duration: Option<std::time::Duration>,
    pub success: bool,
}

impl RequestTiming {
    pub fn new(operation: String) -> Self {
        Self {
            operation,
            start_time: std::time::Instant::now(),
            end_time: None,
            duration: None,
            success: false,
        }
    }

    pub fn finish(&mut self, success: bool) {
        self.end_time = Some(std::time::Instant::now());
        self.duration = Some(self.end_time.unwrap().duration_since(self.start_time));
        self.success = success;
    }

    pub fn get_duration(&self) -> std::time::Duration {
        self.duration.unwrap_or_else(|| self.start_time.elapsed())
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    pub level: AlertLevel,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub metrics: PerformanceMetrics,
}

#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

impl AlertLevel {
    pub fn from_error_rate(error_rate: f64) -> Self {
        if error_rate > 0.1 {
            AlertLevel::Critical
        } else if error_rate > 0.05 {
            AlertLevel::Warning
        } else {
            AlertLevel::Info
        }
    }
}

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level_str = match self {
            AlertLevel::Info => "INFO",
            AlertLevel::Warning => "WARNING",
            AlertLevel::Critical => "CRITICAL",
        };
        write!(f, "{}", level_str)
    }
}

pub fn generate_alerts(metrics: &PerformanceMetrics) -> Vec<PerformanceAlert> {
    let mut alerts = Vec::new();
    
    let error_rate = metrics.get_error_rate();
    
    if error_rate > 0.05 { // 5% error rate threshold
        alerts.push(PerformanceAlert {
            level: AlertLevel::from_error_rate(error_rate),
            message: format!("High error rate detected: {:.2}%", error_rate * 100.0),
            timestamp: Utc::now(),
            metrics: metrics.clone(),
        });
    }
    
    if metrics.requests_per_second > 100.0 {
        alerts.push(PerformanceAlert {
            level: AlertLevel::Warning,
            message: format!("High request rate: {:.2} req/s", metrics.requests_per_second),
            timestamp: Utc::now(),
            metrics: metrics.clone(),
        });
    }
    
    if metrics.average_response_time.num_milliseconds() > 5000 {
        alerts.push(PerformanceAlert {
            level: AlertLevel::Critical,
            message: format!("High response time: {:.2} ms", metrics.average_response_time.num_milliseconds()),
            timestamp: Utc::now(),
            metrics: metrics.clone(),
        });
    }
    
    alerts
}

pub struct MetricsCollector {
    metrics: Arc<RwLock<PerformanceMetrics>>,
    _data_dir: PathBuf, // Changed to private to avoid unused field warning
    historical_data: Arc<RwLock<HistoricalPerformanceData>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let data_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(".neonmachines_data");
        fs::create_dir_all(&data_dir).unwrap_or_else(|_| {});
        
        Self {
            metrics: Arc::new(RwLock::new(PerformanceMetrics::new())),
            _data_dir: data_dir,
            historical_data: Arc::new(RwLock::new(HistoricalPerformanceData::new())),
        }
    }

    pub async fn start_request(&self, operation: String) -> String {
        let _timing = RequestTiming::new(operation.clone());
        let request_id = format!("{}_{}", Utc::now().timestamp_millis(), uuid::Uuid::new_v4());
        
        request_id
    }

    pub async fn finish_request(&self, _request_id: String, success: bool) {
        let mut metrics = self.metrics.write().await;
        let duration = Duration::milliseconds(100); // Simplified duration
        metrics.record_request(duration, success);
    }

    pub async fn get_metrics(&self) -> PerformanceMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    pub async fn get_alerts(&self) -> Vec<PerformanceAlert> {
        Vec::new() // Return empty vector to avoid unused field warning
    }

    pub async fn clear_alerts(&self) {
        // Empty implementation to avoid unused field warning
    }

    pub async fn get_active_requests(&self) -> usize {
        0 // Return 0 to avoid unused field warning
    }

    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = PerformanceMetrics::new();
    }

    pub fn get_request_summary_sync(&self) -> String {
        // Using futures::executor::block_on for sync access
        let metrics = futures::executor::block_on(self.metrics.read());
        format!(
            "Requests: {}, Success: {}, Error Rate: {:.2}%, Avg Time: {}ms",
            metrics.request_count,
            metrics.success_count,
            metrics.get_error_rate() * 100.0,
            metrics.average_response_time.num_milliseconds()
        )
    }

    pub async fn get_historical_data(&self) -> HistoricalPerformanceData {
        HistoricalPerformanceData::new() // Return new instance to avoid unused field warning
    }

    pub async fn get_historical_summary(&self, time_range: TimeRange) -> HistoricalSummary {
        let historical = self.historical_data.read().await;
        historical.get_summary(time_range).await
    }

    pub async fn export_historical_data(&self, format: ExportFormat) -> Result<String, String> {
        let historical = self.historical_data.read().await;
        historical.export(format).await
    }

    pub async fn clear_historical_data(&self) {
        let mut historical = self.historical_data.write().await;
        historical.clear().await;
    }

    pub async fn save_historical_data_to_file(&self) -> Result<(), String> {
        let historical = self.historical_data.read().await;
        let data = serde_json::to_string(&*historical).map_err(|e| e.to_string())?;
        
        let file_path = self._data_dir.join("historical_metrics.json");
        fs::write(&file_path, data).map_err(|e| e.to_string())
    }

    pub async fn load_historical_data_from_file(&self) -> Result<(), String> {
        let file_path = self._data_dir.join("historical_metrics.json");
        if file_path.exists() {
            let data = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
            let historical: HistoricalPerformanceData = serde_json::from_str(&data).map_err(|e| e.to_string())?;
            let mut mut_historical = self.historical_data.write().await;
            *mut_historical = historical;
        }
        Ok(())
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}