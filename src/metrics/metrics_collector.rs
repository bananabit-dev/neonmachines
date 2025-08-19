use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, TimeZone, Duration};
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use crate::metrics::{PerformanceMetrics, RequestTiming, PerformanceAlert};

/// Time range for historical data queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeRange {
    LastHour,
    Last6Hours,
    Last24Hours,
    Last7Days,
    Last30Days,
    All,
}

/// Export format for historical data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Csv,
}

/// Historical performance data entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalEntry {
    pub timestamp: DateTime<Utc>,
    pub metrics: PerformanceMetrics,
    pub operation: String,
    pub success: bool,
}

/// Summary statistics for historical data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalSummary {
    pub total_requests: u64,
    pub average_response_time_ms: f64,
    pub success_rate_percent: f64,
    pub total_duration_ms: u64,
    pub min_response_time_ms: u64,
    pub max_response_time_ms: u64,
    pub request_rate_per_second: f64,
    pub time_range_start: DateTime<Utc>,
    pub time_range_end: DateTime<Utc>,
}

/// Historical performance data storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalPerformanceData {
    pub entries: Vec<HistoricalEntry>,
    pub max_entries: usize,
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
            metrics,
            operation: "workflow_execution".to_string(),
            success: true, // Can be enhanced to track per-operation success
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
            TimeRange::Last6Hours => now - Duration::hours(6),
            TimeRange::Last24Hours => now - Duration::hours(24),
            TimeRange::Last7Days => now - Duration::days(7),
            TimeRange::Last30Days => now - Duration::days(30),
            TimeRange::All => self.entries.first().map(|e| e.timestamp).unwrap_or(now),
        };

        let filtered_entries: Vec<&HistoricalEntry> = self.entries
            .iter()
            .filter(|entry| entry.timestamp >= start_time)
            .collect();

        if filtered_entries.is_empty() {
            return HistoricalSummary {
                total_requests: 0,
                average_response_time_ms: 0.0,
                success_rate_percent: 0.0,
                total_duration_ms: 0,
                min_response_time_ms: 0,
                max_response_time_ms: 0,
                request_rate_per_second: 0.0,
                time_range_start: start_time,
                time_range_end: now,
            };
        }

        let total_requests = filtered_entries.iter().map(|e| e.metrics.request_count).sum();
        let total_duration_ms = filtered_entries.iter().map(|e| e.metrics.total_duration.as_millis() as u64).sum();
        let success_count = filtered_entries.iter().map(|e| e.metrics.success_count).sum();
        let average_response_time_ms = filtered_entries.iter().map(|e| e.metrics.average_response_time.as_millis() as f64).sum::<f64>() / filtered_entries.len() as f64;
        
        let min_response_time_ms = filtered_entries.iter().map(|e| e.metrics.average_response_time.as_millis()).min().unwrap_or(0);
        let max_response_time_ms = filtered_entries.iter().map(|e| e.metrics.average_response_time.as_millis()).max().unwrap_or(0);
        
        let duration_seconds = (now - start_time).num_seconds() as f64;
        let request_rate_per_second = if duration_seconds > 0.0 {
            total_requests as f64 / duration_seconds
        } else {
            0.0
        };

        HistoricalSummary {
            total_requests,
            average_response_time_ms,
            success_rate_percent: if total_requests > 0 { (success_count as f64 / total_requests as f64) * 100.0 } else { 0.0 },
            total_duration_ms,
            min_response_time_ms,
            max_response_time_ms,
            request_rate_per_second,
            time_range_start: start_time,
            time_range_end: now,
        }
    }

    pub async fn export(&self, format: ExportFormat) -> Result<String, String> {
        match format {
            ExportFormat::Json => {
                Ok(serde_json::to_string_pretty(self).map_err(|e| e.to_string())?)
            }
            ExportFormat::Csv => {
                let mut csv = String::new();
                csv.push_str("Timestamp,Operation,Success,RequestCount,SuccessCount,ErrorCount,AvgResponseTimeMs,TotalDurationMs,RPS,MemoryUsageMB,CPUUsagePercent\n");
                
                for entry in &self.entries {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},{},{}\n",
                        entry.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
                        entry.operation,
                        entry.success,
                        entry.metrics.request_count,
                        entry.metrics.success_count,
                        entry.metrics.error_count,
                        entry.metrics.average_response_time.as_millis(),
                        entry.metrics.total_duration.as_millis(),
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
}

impl Default for HistoricalPerformanceData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct MetricsCollector {
    metrics: Arc<RwLock<PerformanceMetrics>>,
    request_timings: Arc<RwLock<HashMap<String, RequestTiming>>>,
    alerts: Arc<RwLock<Vec<PerformanceAlert>>>,
    historical_data: Arc<RwLock<HistoricalPerformanceData>>,
    data_dir: PathBuf,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let data_dir = PathBuf::from("metrics_data");
        fs::create_dir_all(&data_dir).unwrap_or_else(|_| {});
        
        Self {
            metrics: Arc::new(RwLock::new(PerformanceMetrics::new())),
            request_timings: Arc::new(RwLock::new(HashMap::new())),
            alerts: Arc::new(RwLock::new(Vec::new())),
            historical_data: Arc::new(RwLock::new(HistoricalPerformanceData::new())),
            data_dir,
        }
    }

    pub async fn start_request(&self, operation: String) -> String {
        let timing = RequestTiming::new(operation.clone());
        let request_id = format!("{}_{}", Utc::timestamp_millis(&Utc::now()), uuid::Uuid::new_v4());
        
        let mut timings = self.request_timings.write().await;
        timings.insert(request_id.clone(), timing);
        
        request_id
    }

    pub async fn finish_request(&self, request_id: String, success: bool) {
        let mut timings = self.request_timings.write().await;
        if let Some(mut timing) = timings.remove(&request_id) {
            timing.finish(success);
            
            let duration = timing.get_duration();
            let mut metrics = self.metrics.write().await;
            metrics.record_request(duration, success);
            
            // Update historical data
            let mut historical = self.historical_data.write().await;
            historical.add_metrics_snapshot(metrics.clone()).await;
            
            // Generate alerts based on updated metrics
            let alerts = crate::metrics::generate_alerts(&metrics);
            let mut alerts_vec = self.alerts.write().await;
            alerts_vec.extend(alerts);
        }
    }

    pub async fn get_metrics(&self) -> PerformanceMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    pub async fn get_alerts(&self) -> Vec<PerformanceAlert> {
        let alerts = self.alerts.read().await;
        alerts.clone()
    }

    pub async fn clear_alerts(&self) {
        let mut alerts = self.alerts.write().await;
        alerts.clear();
    }

    pub async fn get_active_requests(&self) -> usize {
        let timings = self.request_timings.read().await;
        timings.len()
    }

    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = PerformanceMetrics::new();
        
        let mut alerts = self.alerts.write().await;
        alerts.clear();
    }

    pub async fn get_request_summary(&self) -> String {
        let metrics = self.metrics.read().await;
        format!(
            "Requests: {}, Success Rate: {:.1}%, Avg Time: {:.2}ms, Active: {}",
            metrics.request_count,
            metrics.get_success_rate() * 100.0,
            metrics.average_response_time.as_millis(),
            self.get_active_requests().await
        )
    }

    pub async fn get_historical_data(&self) -> HistoricalPerformanceData {
        let historical = self.historical_data.read().await;
        historical.clone()
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
        let data = serde_json::to_string(&historical).map_err(|e| e.to_string())?;
        let file_path = self.data_dir.join("historical_metrics.json");
        fs::write(&file_path, data).map_err(|e| e.to_string())
    }

    pub async fn load_historical_data_from_file(&self) -> Result<(), String> {
        let file_path = self.data_dir.join("historical_metrics.json");
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
