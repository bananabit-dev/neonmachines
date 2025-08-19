use ratatui::text::{Line, Span};
use ratatui::style::{Style, Color, Modifier};
use ratatui::widgets::{Block, Borders, Gauge as RatatuiGauge, Paragraph, Wrap, BarChart as RatatuiBarChart};
use ratatui::layout::{Rect, Alignment};
use chrono::{DateTime, Utc, Duration};
use std::time;

// Re-export historical data types from metrics_collector
pub use crate::metrics::metrics_collector::{HistoricalEntry, HistoricalSummary, TimeRange};

#[derive(Debug, Clone)]
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
        let elapsed = Utc::now() - self.timestamp;
        let elapsed_seconds = elapsed.num_seconds() as f64;
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

/// Chart rendering functions for performance dashboard
pub mod charts {
    use super::*;
    use ratatui::widgets::{Gauge, BarChart};
    
    /// Render a gauge widget for CPU usage
    pub fn cpu_gauge(metrics: &PerformanceMetrics, _area: Rect) -> Gauge {
        let percentage = metrics.cpu_usage_percent.min(100.0).max(0.0);
        let label = format!("CPU: {:.1}%", percentage);
        
        RatatuiGauge::default()
            .block(Block::default().borders(Borders::ALL).title("CPU Usage"))
            .gauge_style(Style::default().fg(Color::Blue))
            .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .ratio(percentage / 100.0)
            .label(label)
    }

    /// Render a gauge widget for memory usage
    pub fn memory_gauge(metrics: &PerformanceMetrics, _area: Rect) -> Gauge {
        let percentage = (metrics.memory_usage_mb / 1000.0 * 100.0).min(100.0).max(0.0);
        let label = format!("Memory: {:.1}MB", percentage);
        
        RatatuiGauge::default()
            .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
            .gauge_style(Style::default().fg(Color::Green))
            .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            .ratio(percentage / 100.0)
            .label(label)
    }

    /// Render a bar chart for request statistics
    pub fn request_stats_chart(metrics: &PerformanceMetrics, _area: Rect) -> BarChart {
        let success_rate = metrics.get_success_rate();
        let error_rate = metrics.get_error_rate();
        
        let data = vec![
            ("Success", (success_rate * 100.0) as u64),
            ("Error", (error_rate * 100.0) as u64),
        ];

        RatatuiBarChart::default()
            .block(Block::default().borders(Borders::ALL).title("Request Success Rate"))
            .bar_width(10)
            .bar_style(Style::default().fg(Color::White))
            .value_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .data(&data)
    }

    /// Render a line chart for requests per second (simplified bar chart representation)
    pub fn requests_per_second_chart(metrics: &PerformanceMetrics, _area: Rect) -> BarChart {
        let data = vec![
            ("RPS", metrics.requests_per_second as u64),
        ];

        RatatuiBarChart::default()
            .block(Block::default().borders(Borders::ALL).title("Requests per Second"))
            .bar_width(15)
            .bar_style(Style::default().fg(Color::Blue))
            .value_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .data(&data)
    }

    /// Render response time metrics as a paragraph with styling
    pub fn response_time_metrics(metrics: &PerformanceMetrics, _area: Rect) -> Paragraph {
        let mut spans = Vec::new();
        
        spans.push(Span::styled("Average: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}ms", metrics.average_response_time.num_milliseconds()), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("P95: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}ms", metrics.p95_response_time.num_milliseconds()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("P99: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}ms", metrics.p99_response_time.num_milliseconds()), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));

        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title("Response Times"))
            .alignment(Alignment::Left)
    }

    /// Render overall performance summary
    pub fn performance_summary(metrics: &PerformanceMetrics, _area: Rect) -> Paragraph {
        let success_rate = metrics.get_success_rate() * 100.0;
        let mut spans = Vec::new();
        
        spans.push(Span::styled("Total Requests: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{}", metrics.request_count), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("Success Rate: ", Style::default().fg(Color::White)));
        
        let success_style = if success_rate >= 95.0 {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else if success_rate >= 90.0 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        };
        
        spans.push(Span::styled(format!("{:.1}%", success_rate), success_style));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("Total Time: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}s", metrics.total_duration.num_seconds()), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));

        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title("Performance Summary"))
    }

    /// Dashboard mode enum for different dashboard views
    #[derive(Debug, Clone)]
    pub enum DashboardMode {
        Overview,
        Historical,
        Alerts,
    }

    /// Get dashboard title based on current mode
    pub fn get_dashboard_title(mode: &DashboardMode) -> String {
        match mode {
            DashboardMode::Overview => "Performance Overview".to_string(),
            DashboardMode::Historical => "Historical Performance".to_string(),
            DashboardMode::Alerts => "Performance Alerts".to_string(),
        }
    }

    /// Render historical performance data as a line chart
    pub fn historical_trend_chart(historical_data: &crate::metrics::metrics_collector::HistoricalPerformanceData, _area: Rect) -> BarChart {
        use chrono::{Duration, Utc};
        
        // Group data by time intervals for visualization
        let now = Utc::now();
        let one_hour_ago = now - Duration::hours(1);
        
        let recent_data: Vec<&crate::metrics::metrics_collector::HistoricalEntry> = historical_data.get_entries()
            .iter()
            .filter(|entry| *entry.timestamp() >= one_hour_ago)
            .collect();

        if recent_data.is_empty() {
            return RatatuiBarChart::default()
                .block(Block::default().borders(Borders::ALL).title("Historical Trends (Last Hour) - No Data"))
                .bar_width(10)
                .data(&[]);
        }

        // Group data into 5-minute intervals and calculate average metrics
        let mut intervals = Vec::new();
        for i in 0..12 {
            let current_interval_start = one_hour_ago + Duration::minutes(i * 5);
            let interval_end = current_interval_start + Duration::minutes(5);
            
            let interval_data: Vec<&crate::metrics::metrics_collector::HistoricalEntry> = recent_data
                .iter()
                .filter(|entry| *entry.timestamp() >= current_interval_start && *entry.timestamp() < interval_end)
                .cloned()
                .collect();
                
            if !interval_data.is_empty() {
                let avg_response_time: f64 = interval_data
                    .iter()
                    .map(|e| e.metrics().average_response_time.num_milliseconds() as f64)
                    .sum::<f64>() / interval_data.len() as f64;
                    
                intervals.push((current_interval_start, avg_response_time));
            }
        }

        // Convert to data for display (simplified representation)
        let data: Vec<(&str, u64)> = intervals
            .into_iter()
            .enumerate()
            .map(|(i, (_, response_time))| {
                // Create static strings for labels - this is a workaround for now
                match i {
                    0 => ("1", response_time as u64),
                    1 => ("2", response_time as u64),
                    2 => ("3", response_time as u64),
                    3 => ("4", response_time as u64),
                    4 => ("5", response_time as u64),
                    5 => ("6", response_time as u64),
                    6 => ("7", response_time as u64),
                    7 => ("8", response_time as u64),
                    8 => ("9", response_time as u64),
                    9 => ("10", response_time as u64),
                    10 => ("11", response_time as u64),
                    11 => ("12", response_time as u64),
                    _ => ("", response_time as u64),
                }
            })
            .collect();

        RatatuiBarChart::default()
            .block(Block::default().borders(Borders::ALL).title("Historical Response Times (Last Hour)"))
            .bar_width(8)
            .data(&data)
    }

    /// Render historical performance summary
    pub fn historical_summary(summary: &HistoricalSummary, _area: Rect) -> Paragraph {
        let mut spans = Vec::new();
        
        spans.push(Span::styled("Time Range: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(
            format!("{} to {}",
                summary.start_time.format("%H:%M:%S"),
                summary.end_time.format("%H:%M:%S")
            ),
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
        ));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("Total Requests: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{}", summary.total_requests), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw("\n"));
        
        let success_rate = summary.success_rate_percent;
        let success_style = if success_rate >= 95.0 {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else if success_rate >= 90.0 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        };
        
        spans.push(Span::styled("Success Rate: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.1}%", success_rate), success_style));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("Avg Response Time: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}ms", summary.average_response_time_ms), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("Request Rate: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}/s", summary.request_rate_per_second), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("Min/Max Response: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(
            format!("{:.2}/{:.2}ms", summary.min_response_time_ms, summary.max_response_time_ms),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        ));

        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title("Historical Summary"))
    }

    /// Render time range selector for historical data
    pub fn time_range_selector(current_range: &TimeRange, _area: Rect) -> Paragraph {
        let mut spans = Vec::new();
        spans.push(Span::styled("Time Range: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
        
        let options = vec![
            ("Last Hour", TimeRange::LastHour),
            ("Last Day", TimeRange::LastDay),
            ("Last Week", TimeRange::LastWeek),
            ("All", TimeRange::All),
        ];
        
        for (label, range) in options {
            let style = if current_range == &range {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            spans.push(Span::styled(label, style));
            spans.push(Span::raw(" | "));
        }
        
        // Remove trailing " | "
        if spans.len() > 2 {
            spans.pop();
        }

        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title("Historical Data Time Range"))
    }

    /// Export options for historical data
    pub fn export_options(_area: Rect) -> Paragraph<'static> {
        let mut spans = Vec::new();
        spans.push(Span::styled("Export Options: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw("[J]son | [C]SV | [S]ave to file | [L]oad from file"));

        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title("Export & Save Options"))
    }
}