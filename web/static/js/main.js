console.log("main.js loaded");
document.addEventListener('DOMContentLoaded', () => {
    console.log("DOM fully loaded and parsed");
    const chatMessages = document.getElementById('chat-messages');
    const chatInput = document.getElementById('chat-input');
    const sendBtn = document.getElementById('send-btn');
    const tabs = document.querySelectorAll('.nav-tab');
    const tabContents = document.querySelectorAll('.tab-content');

    const socket = new WebSocket('ws://' + location.host + '/ws');

    socket.onopen = () => {
        addMessage('system', 'Connected to the server.');
    };

    socket.onmessage = (event) => {
        const message = event.data;
        addMessage('server', message);
    };

    socket.onclose = () => {
        addMessage('system', 'Disconnected from the server.');
    };

    // Tab switching
    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            console.log(`Tab clicked: ${tab.getAttribute('data-tab')}`);
            tabs.forEach(t => t.classList.remove('active'));
            tab.classList.add('active');
            const tabName = tab.getAttribute('data-tab');
            tabContents.forEach(content => {
                if (content.id === tabName) {
                    content.classList.add('active');
                } else {
                    content.classList.remove('active');
                }
            });
            loadTabContent(tabName);
        });
    });

    // Chat functionality
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
            socket.send(inputText);
            chatInput.value = '';
        }
    }

    sendBtn.addEventListener('click', handleUserInput);
    chatInput.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') {
            handleUserInput();
        }
    });

    // Welcome message
    addMessage('system', 'Welcome to Neonmachines! Type your message or use /help for commands.');

    const loadedScripts = new Set();

    function loadTabContent(tabName) {
        console.log(`Loading content for tab: ${tabName}`);
        const contentDiv = document.getElementById(tabName);
        
        // Only load content for non-chat tabs
        if (tabName !== 'chat') {
            fetch(`${tabName}.html`)
                .then(response => {
                    if (!response.ok) {
                        throw new Error(`Failed to load ${tabName}.html`);
                    }
                    return response.text();
                })
                .then(data => {
                    contentDiv.innerHTML = data;
                    // Load script only if it hasn't been loaded before
                    if (!loadedScripts.has(tabName)) {
                        console.log(`Loading script for tab: ${tabName}`);
                        const script = document.createElement('script');
                        script.src = `static/js/${tabName}.js`;
                        script.onload = () => {
                            console.log(`Script loaded for tab: ${tabName}`);
                            loadedScripts.add(tabName);
                        };
                        document.body.appendChild(script);
                    }
                })
                .catch(error => {
                    console.error('Error loading tab content:', error);
                    contentDiv.innerHTML = `<p class="error-message">Error loading content for ${tabName}.</p>`;
                });
        }
    }

    // Load the default tab's content
    loadTabContent('graph-editor');

    const saveBtn = document.getElementById('save-btn');
    const runBtn = document.getElementById('run-btn');

    saveBtn.addEventListener('click', () => {
        const activeTab = document.querySelector('.nav-tab.active').getAttribute('data-tab');
        if (activeTab === 'poml-editor') {
            const pomlContent = document.getElementById('poml-text-editor').value;
            socket.send(JSON.stringify({ command: 'save', file: 'workflow.poml', content: pomlContent }));
        } else {
            console.log("Save functionality is only available for POML editor.");
        }
    });

    runBtn.addEventListener('click', () => {
        const activeTab = document.querySelector('.nav-tab.active').getAttribute('data-tab');
        socket.send(JSON.stringify({ command: 'run', workflow: activeTab }));
    });
});
