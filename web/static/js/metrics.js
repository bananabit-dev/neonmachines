document.addEventListener('DOMContentLoaded', () => {
    const metricsData = document.getElementById('metrics-data');

    function fetchMetrics() {
        fetch('/api/metrics')
            .then(response => response.json())
            .then(data => {
                metricsData.innerHTML = `
                    <p><strong>Uptime:</strong> ${data.uptime}</p>
                    <p><strong>Memory Usage:</strong> ${data.memory_usage}</p>
                    <p><strong>CPU Usage:</strong> ${data.cpu_usage}</p>
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
