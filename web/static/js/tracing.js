document.addEventListener('DOMContentLoaded', () => {
    const tracesContainer = document.getElementById('traces-container');

    function fetchTraces() {
        fetch('/api/tracing')
            .then(response => response.json())
            .then(data => {
                if (data.length === 0) {
                    tracesContainer.innerHTML = '<p class="loading">No traces available.</p>';
                    return;
                }

                tracesContainer.innerHTML = data.map(trace => `
                    <div class="trace-item ${trace.status.toLowerCase() === 'failure' ? 'failure' : ''}">
                        <div class="trace-header">
                            <div class="trace-service ${trace.status.toLowerCase() === 'failure' ? 'failure' : ''}">
                                ${trace.service}
                            </div>
                            <div class="trace-duration">
                                ${trace.duration}
                            </div>
                        </div>
                        <div class="trace-id">
                            ID: ${trace.id}
                        </div>
                        <div class="trace-timestamp">
                            ${trace.timestamp}
                        </div>
                        <div class="trace-details ${trace.status.toLowerCase() === 'failure' ? 'failure' : ''}">
                            ${trace.details}
                        </div>
                    </div>
                `).join('');
            })
            .catch(error => {
                console.error('Error fetching traces:', error);
                tracesContainer.innerHTML = '<p class="error-message">Error fetching traces.</p>';
            });
    }

    // Fetch traces every 5 seconds
    setInterval(fetchTraces, 5000);

    // Initial fetch
    fetchTraces();
});
