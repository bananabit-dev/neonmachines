document.addEventListener('DOMContentLoaded', () => {
    const chatMessages = document.getElementById('chat-messages');
    const chatInput = document.getElementById('chat-input');
    const sendBtn = document.getElementById('send-btn');
    const tabs = document.querySelectorAll('.nav-tab');
    const tabContents = document.querySelectorAll('.tab-content');

    const socket = new WebSocket('ws://' + location.host + '/ws');

    socket.onopen = () => addMessage('system', 'Connected to the server.');
    socket.onclose = () => addMessage('system', 'Disconnected from the server.');
    socket.onerror = (error) => {
        console.error('WebSocket error:', error);
        addMessage('system', 'Error connecting to the server.');
    };

    // Make the socket connection globally available for other scripts
    window.socket = socket;

    // Handle incoming messages from the server
    socket.onmessage = (event) => {
        try {
            const response = JSON.parse(event.data);
            const from = response.status || 'server';
            const text = response.data || event.data;
            addMessage(from, text);
        } catch (e) {
            // If the message is not JSON, display it as-is
            addMessage('server', event.data);
        }
    };

    // --- Tab Switching Logic ---
    const loadedTabs = new Set();
    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            tabs.forEach(t => t.classList.remove('active'));
            tab.classList.add('active');
            const tabName = tab.getAttribute('data-tab');
            tabContents.forEach(content => {
                content.classList.toggle('active', content.id === tabName);
            });
            loadTabContent(tabName);
        });
    });

    function loadTabContent(tabName) {
        // Chat tab is static, and other tabs should only be loaded once
        if (tabName === 'chat' || loadedTabs.has(tabName)) {
            return;
        }

        const contentDiv = document.getElementById(tabName);
        fetch(`${tabName}.html`)
            .then(response => {
                if (!response.ok) throw new Error(`Failed to load HTML for ${tabName}`);
                return response.text();
            })
            .then(html => {
                contentDiv.innerHTML = html;
                // Now, dynamically load the corresponding JavaScript for the tab
                const script = document.createElement('script');
                script.src = `static/js/${tabName}.js`;
                document.body.appendChild(script);
                loadedTabs.add(tabName);
            })
            .catch(error => {
                console.error('Error loading tab content:', error);
                contentDiv.innerHTML = `<p>Error loading content.</p>`;
            });
    }

    // --- Chat Functionality ---
    function addMessage(from, text) {
        const messageElement = document.createElement('div');
        messageElement.classList.add('chat-message');
        messageElement.innerHTML = `<span class="message-from">${from}:</span> <span class="message-text">${text}</span>`;
        chatMessages.appendChild(messageElement);
        chatMessages.scrollTop = chatMessages.scrollHeight;
    }

    function handleUserInput() {
        const inputText = chatInput.value.trim();
        if (inputText) {
            addMessage('you', inputText);
            // Send chat messages in the JSON format the backend expects
            socket.send(JSON.stringify({ command: "submit", payload: inputText }));
            chatInput.value = '';
        }
    }

    sendBtn.addEventListener('click', handleUserInput);
    chatInput.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') handleUserInput();
    });

    addMessage('system', 'Welcome to Neonmachines!');
});
