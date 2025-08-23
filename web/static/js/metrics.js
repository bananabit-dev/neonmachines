document.addEventListener('DOMContentLoaded', () => {
    const metricsData = document.getElementById('metrics-data');

    function fetchMetrics() {
        fetch('/api/metrics')
            .then(response => response.json())
            .then(data => {
                metricsData.innerHTML = `
                    <div class="metrics-grid">
                        <div class="metric-card">
                            <h3>Requests</h3>
                            <p class="metric-value">${data.requests_count}</p>
                            <p class="metric-label">Total Requests</p>
                        </div>
                        <div class="metric-card">
                            <h3>Success Rate</h3>
                            <p class="metric-value">${(data.success_rate * 100).toFixed(1)}%</p>
                            <p class="metric-label">Success Rate</p>
                        </div>
                        <div class="metric-card">
                            <h3>Response Time</h3>
                            <p class="metric-value">${data.average_response_time.toFixed(2)}ms</p>
                            <p class="metric-label">Average</p>
                        </div>
                        <div class="metric-card">
                            <h3>Active Requests</h3>
                            <p class="metric-value">${data.active_requests}</p>
                            <p class="metric-label">Currently Processing</p>
                        </div>
                    </div>
                    ${data.alerts.length > 0 ? `
                        <div class="alerts-section">
                            <h3>Alerts</h3>
                            <ul class="alerts-list">
                                ${data.alerts.map(alert => `<li class="alert-item">${alert}</li>`).join('')}
                            </ul>
                        </div>
                    ` : '<p class="no-alerts">No active alerts</p>'}
                `;
            })
            .catch(error => {
                console.error('Error fetching metrics:', error);
                metricsData.innerHTML = '<p class="error-message">Error fetching metrics.</p>';
            });
    }

    // Fetch metrics every 5 seconds
    setInterval(fetchMetrics, 5000);

    // Initial fetch
    fetchMetrics();
});
