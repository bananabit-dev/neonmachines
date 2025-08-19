use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ratatui::prelude::{Style, Stylize, Color, Rect, Alignment};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap, Gauge, BarChart};

// Re-export historical data types from metrics_collector
pub use crate::metrics::metrics_collector::{
    TimeRange,
    ExportFormat,
    HistoricalEntry,
    HistoricalSummary,
    HistoricalPerformanceData,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub request_count: u64,
    pub total_duration: Duration,
    pub success_count: u64,
    pub error_count: u64,
    pub average_response_time: Duration,
    pub p95_response_time: Duration,
    pub p99_response_time: Duration,
    pub requests_per_second: f64,
    pub throughput_mbps: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub timestamp: DateTime<Utc>,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            request_count: 0,
            total_duration: Duration::from_secs(0),
            success_count: 0,
            error_count: 0,
            average_response_time: Duration::from_secs(0),
            p95_response_time: Duration::from_secs(0),
            p99_response_time: Duration::from_secs(0),
            requests_per_second: 0.0,
            throughput_mbps: 0.0,
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
        self.total_duration += duration;
        
        if success {
            self.success_count += 1;
        } else {
            self.error_count += 1;
        }

        self.update_derived_metrics();
    }

    pub fn update_derived_metrics(&mut self) {
        if self.request_count > 0 {
            self.average_response_time = Duration::from_millis((self.total_duration.as_millis() / self.request_count) as u64);
        }

        // Update requests per second
        let elapsed_seconds = self.timestamp.elapsed().as_secs_f64();
        if elapsed_seconds > 0.0 {
            self.requests_per_second = self.request_count as f64 / elapsed_seconds;
        }

        // Simulate memory and CPU usage (in a real implementation, you'd get actual system metrics)
        self.memory_usage_mb = (self.request_count as f64 * 0.1) % 100.0;
        self.cpu_usage_percent = (self.requests_per_second * 0.5).min(100.0);
    }

    pub fn get_success_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            self.success_count as f64 / self.request_count as f64
        }
    }

    pub fn get_error_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            self.error_count as f64 / self.request_count as f64
        }
    }
}

#[derive(Debug, Clone)]
pub struct RequestTiming {
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub duration: Option<Duration>,
    pub operation: String,
    pub success: bool,
}

impl RequestTiming {
    pub fn new(operation: String) -> Self {
        Self {
            start_time: Instant::now(),
            end_time: None,
            duration: None,
            operation,
            success: false,
        }
    }

    pub fn finish(&mut self, success: bool) {
        self.end_time = Some(Instant::now());
        self.duration = Some(self.end_time.unwrap().duration_since(self.start_time));
        self.success = success;
    }

    pub fn get_duration(&self) -> Duration {
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

#[derive(Debug, Clone, PartialEq)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

impl AlertLevel {
    pub fn from_error_rate(error_rate: f64) -> Self {
        match error_rate {
            rate if rate >= 0.5 => AlertLevel::Critical,
            rate if rate >= 0.2 => AlertLevel::Error,
            rate if rate >= 0.1 => AlertLevel::Warning,
            _ => AlertLevel::Info,
        }
    }
}

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level_str = match self {
            AlertLevel::Info => "INFO",
            AlertLevel::Warning => "WARNING",
            AlertLevel::Error => "ERROR",
            AlertLevel::Critical => "CRITICAL",
        };
        write!(f, "{}", level_str)
    }
}

pub fn generate_alerts(metrics: &PerformanceMetrics) -> Vec<PerformanceAlert> {
    let mut alerts = Vec::new();
    
    let error_rate = metrics.get_error_rate();
    let alert_level = AlertLevel::from_error_rate(error_rate);
    
    if error_rate > 0.05 { // 5% error rate threshold
        alerts.push(PerformanceAlert {
            level: alert_level,
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

    if metrics.average_response_time.as_millis() > 5000 {
        alerts.push(PerformanceAlert {
            level: AlertLevel::Warning,
            message: format!("High response time: {:.2} ms", metrics.average_response_time.as_millis()),
            timestamp: Utc::now(),
            metrics: metrics.clone(),
        });
    }

    alerts
}

/// Chart rendering functions for performance dashboard
pub mod charts {
    use super::*;
    use ratatui::layout::{Rect, Alignment};
    use ratatui::text::{Span, Text};
    use ratatui::widgets::{Block, Borders, Paragraph, Gauge as RatatuiGauge, BarChart as RatatuiBarChart, Bar};
    use ratatui::style::{Modifier, Style};

    /// Render a gauge widget for CPU usage
    pub fn cpu_gauge(metrics: &PerformanceMetrics, area: Rect) -> Gauge {
        let percentage = metrics.cpu_usage_percent;
        let label = format!("CPU: {:.1}%", percentage);
        
        RatatuiGauge::default()
            .block(Block::default().borders(Borders::ALL).title("CPU Usage"))
            .gauge_style(Style::default().fg(Color::Blue))
            .label_style(Style::default().fg(Color::White).bold())
            .ratio(percentage / 100.0)
            .label(label)
    }

    /// Render a gauge widget for memory usage
    pub fn memory_gauge(metrics: &PerformanceMetrics, area: Rect) -> Gauge {
        let percentage = metrics.memory_usage_mb;
        let label = format!("Memory: {:.1}MB", percentage);
        
        RatatuiGauge::default()
            .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
            .gauge_style(Style::default().fg(Color::Green))
            .label_style(Style::default().fg(Color::White).bold())
            .ratio(percentage / 100.0)
            .label(label)
    }

    /// Render a bar chart for request statistics
    pub fn request_stats_chart(metrics: &PerformanceMetrics, area: Rect) -> BarChart {
        let success_rate = metrics.get_success_rate();
        let error_rate = metrics.get_error_rate();
        
        let bars = vec![
            Bar::new("Success", (success_rate * 100.0) as u64)
                .style(Style::default().fg(Color::Green)),
            Bar::new("Error", (error_rate * 100.0) as u64)
                .style(Style::default().fg(Color::Red)),
        ];
        
        RatatuiBarChart::default()
            .block(Block::default().borders(Borders::ALL).title("Request Success Rate"))
            .bar_width(10)
            .bar_style(Style::default().fg(Color::White))
            .value_style(Style::default().fg(Color::Yellow).bold())
            .bars(&bars)
    }

    /// Render a line chart for requests per second (simplified bar chart representation)
    pub fn requests_per_second_chart(metrics: &PerformanceMetrics, area: Rect) -> BarChart {
        let bars = vec![
            Bar::new("RPS", metrics.requests_per_second as u64)
                .style(Style::default().fg(Color::Cyan)),
        ];
        
        RatatuiBarChart::default()
            .block(Block::default().borders(Borders::ALL).title("Requests per Second"))
            .bar_width(15)
            .bar_style(Style::default().fg(Color::White))
            .value_style(Style::default().fg(Color::Yellow).bold())
            .bars(&bars)
    }

    /// Render response time metrics as a paragraph with styling
    pub fn response_time_metrics(metrics: &PerformanceMetrics, area: Rect) -> Paragraph {
        let mut spans = Vec::new();
        spans.push(Span::styled("Average: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}ms", metrics.average_response_time.as_millis()), Style::default().fg(Color::Green).bold()));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("P95: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}ms", metrics.p95_response_time.as_millis()), Style::default().fg(Color::Yellow).bold()));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("P99: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}ms", metrics.p99_response_time.as_millis()), Style::default().fg(Color::Red).bold()));
        
        let text = Text::from(Line::from(spans));
        
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Response Times"))
            .alignment(Alignment::Left)
    }

    /// Render overall performance summary
    pub fn performance_summary(metrics: &PerformanceMetrics, area: Rect) -> Paragraph {
        let success_rate = metrics.get_success_rate() * 100.0;
        let mut spans = Vec::new();
        spans.push(Span::styled("Total Requests: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{}", metrics.request_count), Style::default().fg(Color::Blue).bold()));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("Success Rate: ", Style::default().fg(Color::White)));
        let success_style = if success_rate >= 95.0 {
            Style::default().fg(Color::Green).bold()
        } else if success_rate >= 80.0 {
            Style::default().fg(Color::Yellow).bold()
        } else {
            Style::default().fg(Color::Red).bold()
        };
        spans.push(Span::styled(format!("{:.1}%", success_rate), success_style));
        spans.push(Span::raw("\n"));
        spans.push(Span::styled("Total Time: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}s", metrics.total_duration.as_secs_f64()), Style::default().fg(Color::Cyan).bold()));
        
        let text = Text::from(Line::from(spans));
        
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Performance Summary"))
            .alignment(Alignment::Left)
    }

    /// Dashboard mode enum for different dashboard views
    #[derive(Debug, Clone, PartialEq)]
    pub enum DashboardMode {
        Overview,
        Detailed,
        Alerts,
        Historical,
    }

    /// Get dashboard title based on current mode
    pub fn get_dashboard_title(mode: &DashboardMode) -> String {
        match mode {
            DashboardMode::Overview => "📊 Performance Dashboard - Overview",
            DashboardMode::Detailed => "📊 Performance Dashboard - Detailed",
            DashboardMode::Alerts => "📊 Performance Dashboard - Alerts",
            DashboardMode::Historical => "📊 Performance Dashboard - Historical",
        }.to_string()
    }

    /// Render historical performance data as a line chart
    pub fn historical_trend_chart(historical_data: &HistoricalPerformanceData, area: Rect) -> BarChart {
        // Group data by time intervals for visualization
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);
        
        let recent_data: Vec<&HistoricalEntry> = historical_data.entries
            .iter()
            .filter(|entry| entry.timestamp >= one_hour_ago)
            .collect();
        
        if recent_data.is_empty() {
            return BarChart::default()
                .block(Block::default().borders(Borders::ALL).title("Historical Trends (Last Hour) - No Data"))
                .bar_width(10)
                .bars(&[]);
        }

        // Group data into 5-minute intervals and calculate average metrics
        let interval_duration = chrono::Duration::minutes(5);
        let mut intervals: Vec<(DateTime<Utc>, f64)> = Vec::new();
        let mut current_interval_start = one_hour_ago;
        
        while current_interval_start < now {
            let interval_end = current_interval_start + interval_duration;
            let interval_data: Vec<&HistoricalEntry> = recent_data
                .iter()
                .filter(|entry| entry.timestamp >= current_interval_start && entry.timestamp < interval_end)
                .collect();
            
            if !interval_data.is_empty() {
                let avg_response_time = interval_data
                    .iter()
                    .map(|e| e.metrics.average_response_time.as_millis() as f64)
                    .sum::<f64>() / interval_data.len() as f64;
                
                intervals.push((current_interval_start, avg_response_time));
            }
            
            current_interval_start = interval_end;
        }

        // Convert to bars for display (simplified representation)
        let bars: Vec<Bar> = intervals
            .into_iter()
            .enumerate()
            .map(|(i, (_, response_time))| {
                Bar::new(&format!("{}", i + 1), response_time as u64)
                    .style(Style::default().fg(Color::Magenta))
            })
            .collect();

        BarChart::default()
            .block(Block::default().borders(Borders::ALL).title("Historical Response Times (Last Hour)"))
            .bar_width(8)
            .bar_style(Style::default().fg(Color::White))
            .value_style(Style::default().fg(Color::Yellow).bold())
            .bars(&bars)
    }

    /// Render historical performance summary
    pub fn historical_summary(summary: &HistoricalSummary, area: Rect) -> Paragraph {
        let mut spans = Vec::new();
        spans.push(Span::styled("Time Range: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(
            format!("{} to {}", 
                summary.time_range_start.format("%H:%M:%S"),
                summary.time_range_end.format("%H:%M:%S")
            ),
            Style::default().fg(Color::Blue).bold()
        ));
        spans.push(Span::raw("\n"));
        
        spans.push(Span::styled("Total Requests: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{}", summary.total_requests), Style::default().fg(Color::Cyan).bold()));
        spans.push(Span::raw("\n"));
        
        spans.push(Span::styled("Success Rate: ", Style::default().fg(Color::White)));
        let success_style = if summary.success_rate_percent >= 95.0 {
            Style::default().fg(Color::Green).bold()
        } else if summary.success_rate_percent >= 80.0 {
            Style::default().fg(Color::Yellow).bold()
        } else {
            Style::default().fg(Color::Red).bold()
        };
        spans.push(Span::styled(format!("{:.1}%", summary.success_rate_percent), success_style));
        spans.push(Span::raw("\n"));
        
        spans.push(Span::styled("Avg Response Time: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}ms", summary.average_response_time_ms), Style::default().fg(Color::Green).bold()));
        spans.push(Span::raw("\n"));
        
        spans.push(Span::styled("Request Rate: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(format!("{:.2}/s", summary.request_rate_per_second), Style::default().fg(Color::Cyan).bold()));
        spans.push(Span::raw("\n"));
        
        spans.push(Span::styled("Min/Max Response: ", Style::default().fg(Color::White)));
        spans.push(Span::styled(
            format!("{:.2}/{:.2}ms", summary.min_response_time_ms, summary.max_response_time_ms),
            Style::default().fg(Color::Yellow).bold()
        ));
        
        let text = Text::from(Line::from(spans));
        
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Historical Summary"))
            .alignment(Alignment::Left)
    }

    /// Render time range selector for historical data
    pub fn time_range_selector(current_range: &TimeRange, area: Rect) -> Paragraph {
        let mut spans = Vec::new();
        spans.push(Span::styled("Time Range: ", Style::default().fg(Color::White).bold()));
        
        let options = vec![
            ("Last Hour", TimeRange::LastHour),
            ("Last 6 Hours", TimeRange::Last6Hours),
            ("Last 24 Hours", TimeRange::Last24Hours),
            ("Last 7 Days", TimeRange::Last7Days),
            ("Last 30 Days", TimeRange::Last30Days),
            ("All Time", TimeRange::All),
        ];
        
        for (label, range) in options {
            let style = if current_range == &range {
                Style::default().fg(Color::Green).bold()
            } else {
                Style::default().fg(Color::Gray)
            };
            spans.push(Span::styled(label, style));
            spans.push(Span::raw(" | "));
        }
        
        // Remove trailing " | "
        if spans.len() > 2 {
            spans.pop();
            spans.pop();
        }
        
        let text = Text::from(Line::from(spans));
        
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Historical Data Time Range"))
            .alignment(Alignment::Left)
    }

    /// Export options for historical data
    pub fn export_options(area: Rect) -> Paragraph {
        let mut spans = Vec::new();
        spans.push(Span::styled("Export Options: ", Style::default().fg(Color::White).bold()));
        spans.push(Span::raw("[J]son | [C]SV | [S]ave to file | [L]oad from file"));
        
        let text = Text::from(Line::from(spans));
        
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Export & Save Options"))
            .alignment(Alignment::Left)
    }
}
