document.addEventListener('DOMContentLoaded', () => {
    const chatMessages = document.getElementById('chat-messages');
    const chatInput = document.getElementById('chat-input');
    const sendBtn = document.getElementById('send-btn');
    const tabs = document.querySelectorAll('.nav-tab');
    const tabContents = document.querySelectorAll('.tab-content');
    const userAvatar = document.getElementById('user-avatar');
    const avatarUpload = document.getElementById('avatar-upload');
    const userName = document.getElementById('user-name');

    const socket = new WebSocket('ws://' + location.host + '/ws');

    socket.onopen = () => addMessage('system', 'Connected to the server.');
    socket.onclose = () => addMessage('system', 'Disconnected from the server.');
    socket.onerror = (error) => {
        console.error('WebSocket error:', error);
        addMessage('system', 'Error connecting to the server.');
    };

    // Make the socket connection globally available for other scripts
    window.socket = socket;

    // Handle avatar click to trigger file upload
    if (userAvatar && avatarUpload) {
        userAvatar.addEventListener('click', () => {
            avatarUpload.click();
        });

        avatarUpload.addEventListener('change', (event) => {
            const file = event.target.files[0];
            if (file) {
                const reader = new FileReader();
                reader.onload = (e) => {
                    userAvatar.src = e.target.result;
                    // Save to localStorage
                    localStorage.setItem('userAvatar', e.target.result);
                };
                reader.readAsDataURL(file);
            }
        });

        // Load saved avatar from localStorage
        const savedAvatar = localStorage.getItem('userAvatar');
        if (savedAvatar) {
            userAvatar.src = savedAvatar;
        }
    }

    // Load saved user name from localStorage or set default
    const savedUserName = localStorage.getItem('userName') || 'User';
    if (userName) {
        userName.textContent = savedUserName;
        
        // Allow user to change their name by clicking on it
        userName.addEventListener('click', () => {
            const newName = prompt('Enter your name:', savedUserName);
            if (newName !== null && newName.trim() !== '') {
                const trimmedName = newName.trim();
                userName.textContent = trimmedName;
                localStorage.setItem('userName', trimmedName);
            }
        });
    }

    // Handle incoming messages from the server
    socket.onmessage = (event) => {
        try {
            const response = JSON.parse(event.data);
            const from = response.status || 'server';
            const text = response.data || event.data;
            
            // Handle workflow list responses
            if (response.command === 'workflow_list') {
                if (response.payload && Array.isArray(response.payload)) {
                    const workflowList = response.payload.map(wf => `• ${wf}`).join('\n');
                    addMessage('system', `Available workflows:\n${workflowList}`);
                } else {
                    addMessage('system', 'No workflows available.');
                }
                return;
            }
            
            // Handle workflow selection responses
            if (response.command === 'workflow_selected') {
                if (response.payload) {
                    addMessage('system', `Workflow selected: ${response.payload}`);
                } else {
                    addMessage('system', 'Workflow selection confirmed.');
                }
                return;
            }
            
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
            
            // Show/hide POML file selector for POML Editor tab
            const pomlFileSelector = document.getElementById('poml-file-selector');
            if (pomlFileSelector) {
                pomlFileSelector.style.display = tabName === 'poml-editor' ? 'flex' : 'none';
            }
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
                script.onload = function() {
                    console.log(`Script loaded for ${tabName}`);
                    // Special handling for specific tabs
                    setTimeout(() => {
                        if (tabName === 'graph-editor') {
                            if (typeof initializeGraphEditor === 'function') {
                                console.log('Initializing graph editor from main.js');
                                initializeGraphEditor();
                            } else {
                                console.error('initializeGraphEditor function not found');
                            }
                        } else if (tabName === 'poml-editor') {
                            if (typeof initializePomlEditor === 'function') {
                                console.log('Initializing POML editor from main.js');
                                initializePomlEditor();
                                
                                // Check if there's a pending file to load
                                const pomlFileList = document.getElementById('poml-file-list');
                                if (pomlFileList && pomlFileList.value) {
                                    console.log('Loading pending file:', pomlFileList.value);
                                    setTimeout(() => {
                                        loadPomlFile(pomlFileList.value);
                                    }, 500);
                                }
                            } else {
                                console.error('initializePomlEditor function not found');
                            }
                        } else if (typeof window[`initialize${tabName.charAt(0).toUpperCase() + tabName.slice(1)}`] === 'function') {
                            window[`initialize${tabName.charAt(0).toUpperCase() + tabName.slice(1)}`]();
                        }
                    }, 100);
                };
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
        
        // Use user's name for user messages
        let displayName = from;
        if (from === 'you') {
            displayName = localStorage.getItem('userName') || 'User';
        }
        
        // Add special styling for different message types
        if (from === 'agent' || from === 'result') {
            messageElement.classList.add('message-agent');
        } else if (from === 'system') {
            messageElement.classList.add('message-system');
        } else if (from === 'error') {
            messageElement.classList.add('message-error');
        } else if (from === 'you') {
            messageElement.classList.add('message-you');
        } else if (from === 'security') {
            messageElement.classList.add('message-security');
        }
        
        // Create avatar element
        const avatarElement = document.createElement('img');
        avatarElement.className = 'message-avatar';
        
        // Set avatar based on message type
        if (from === 'you') {
            // User avatar
            const savedAvatar = localStorage.getItem('userAvatar');
            if (savedAvatar) {
                avatarElement.src = savedAvatar;
            } else {
                avatarElement.src = 'static/default-avatar.png'; // Default user avatar
            }
            avatarElement.alt = 'You';
        } else if (from === 'agent' || from === 'result') {
            // Agent avatar
            avatarElement.src = 'static/agent-avatar.png'; // Default agent avatar
            avatarElement.alt = 'Agent';
        } else {
            // System/default avatar
            avatarElement.src = 'static/system-avatar.png'; // Default system avatar
            avatarElement.alt = 'System';
        }
        
        // Create the message content with proper format
        const messageFrom = document.createElement('span');
        messageFrom.className = 'message-from';
        messageFrom.textContent = displayName + ':';
        
        const messageText = document.createElement('span');
        messageText.className = 'message-text';
        messageText.innerHTML = text; // Use innerHTML to support formatted help text
        
        // Create container for avatar and message content
        const avatarContainer = document.createElement('div');
        avatarContainer.className = 'message-avatar-container';
        avatarContainer.appendChild(avatarElement);
        
        const contentContainer = document.createElement('div');
        contentContainer.className = 'message-content-container';
        contentContainer.appendChild(messageFrom);
        contentContainer.appendChild(document.createTextNode(' ')); // Add space between name and message
        contentContainer.appendChild(messageText);
        
        messageElement.appendChild(avatarContainer);
        messageElement.appendChild(contentContainer);
        
        chatMessages.appendChild(messageElement);
        chatMessages.scrollTop = chatMessages.scrollHeight;
    }

    // Help command function
    function showHelp() {
        const helpText = `
<strong>Available Commands:</strong>
• <code>/help</code> - Show this help message
• <code>/help preprompting</code> - Show preprompting guide
• <code>/clear</code> - Clear chat history
• <code>/status</code> - Show system status
• <code>/run</code> - Run all workflows
• <code>/run &lt;workflow_name&gt; [prompt]</code> - Run a specific workflow
• <code>/run all</code> - Run all workflows
• <code>/workflow</code> - List available workflows
• <code>/workflow &lt;name&gt;</code> - Select a workflow
• <code>/create_template &lt;mcp|tool&gt; &lt;name&gt;</code> - Create a new template

<strong>Features:</strong>
• Click on your avatar to change it
• Click on your name to change it
• Use the tabs to switch between different tools
• Graph Editor - Create and edit workflow graphs
• POML Editor - Edit POML configuration files
• Metrics - View system metrics
• Tracing - View request traces

<strong>Keyboard Shortcuts:</strong>
• Enter - Send message
• Ctrl+L - Clear chat (if supported)
        `;
        addMessage('system', helpText);
    }

    // Preprompting helper function
    function showPrepromptingHelp() {
        const helpText = `
<div class="preprompting-help">
  <h4>Preprompting with Secondary Agents</h4>
  <p>You can trigger secondary agents by using the following syntax:</p>
  <code>primary task input2="secondary task here"</code>
  <p>Examples:</p>
  <ul>
    <li>create a fibonacci function input2="optimize for performance with memoization"</li>
    <li>build a web scraper input2="add rate limiting and error handling"</li>
    <li>analyze this code input2="check for security vulnerabilities"</li>
  </ul>
  <p>Special workflows available:</p>
  <ul>
    <li><strong>security_analyzer.poml</strong> - For security vulnerability analysis</li>
    <li><strong>coding_agent.poml</strong> - For multi-agent coding tasks</li>
  </ul>
</div>
        `;
        addMessage('system', helpText);
    }

    function handleUserInput() {
        const inputText = chatInput.value.trim();
        if (inputText) {
            // Handle special commands
            const lowerInput = inputText.toLowerCase();
            
            // Help commands
            if (lowerInput === '/help' || lowerInput === '/?') {
                showHelp();
                chatInput.value = '';
                return;
            }
            
            if (lowerInput === '/help preprompting' || lowerInput === '/preprompting') {
                showPrepromptingHelp();
                chatInput.value = '';
                return;
            }
            
            // Clear command
            if (lowerInput === '/clear') {
                chatMessages.innerHTML = '';
                addMessage('system', 'Chat cleared.');
                chatInput.value = '';
                return;
            }
            
            // Status command
            if (lowerInput === '/status') {
                addMessage('system', 'System is running normally.');
                chatInput.value = '';
                return;
            }
            
            // Run command
            if (lowerInput.startsWith('/run')) {
                const parts = inputText.split(' ');
                if (parts.length === 1) {
                    // Run current workflow command
                    if (socket.readyState === WebSocket.OPEN) {
                        socket.send(JSON.stringify({ command: "run_all_workflows", payload: "" }));
                        addMessage('system', 'Running all workflows...');
                    } else {
                        addMessage('error', 'Not connected to server. Please refresh the page.');
                    }
                } else if (parts[1] === 'all') {
                    // Run all workflows command
                    if (socket.readyState === WebSocket.OPEN) {
                        socket.send(JSON.stringify({ command: "run_all_workflows", payload: "" }));
                        addMessage('system', 'Running all workflows...');
                    } else {
                        addMessage('error', 'Not connected to server. Please refresh the page.');
                    }
                } else {
                    // Run specific workflow command
                    const workflowName = parts[1];
                    let prompt = "";
                    if (parts.length > 2) {
                        prompt = parts.slice(2).join(' ');
                    }
                    
                    if (socket.readyState === WebSocket.OPEN) {
                        socket.send(JSON.stringify({ 
                            command: "run_workflow", 
                            payload: {
                                workflow_name: workflowName,
                                prompt: prompt
                            }
                        }));
                        addMessage('system', `Running workflow: ${workflowName}`);
                    } else {
                        addMessage('error', 'Not connected to server. Please refresh the page.');
                    }
                }
                chatInput.value = '';
                return;
            }
            
            // Workflow command
            if (lowerInput.startsWith('/workflow')) {
                const parts = inputText.split(' ');
                if (parts.length === 1) {
                    // List workflows command
                    if (socket.readyState === WebSocket.OPEN) {
                        socket.send(JSON.stringify({ command: "list_workflows", payload: "" }));
                        addMessage('system', 'Requesting workflow list...');
                    } else {
                        addMessage('error', 'Not connected to server. Please refresh the page.');
                    }
                } else {
                    // Select workflow command
                    const workflowName = parts.slice(1).join(' ');
                    if (socket.readyState === WebSocket.OPEN) {
                        socket.send(JSON.stringify({ command: "select_workflow", payload: workflowName }));
                        addMessage('system', `Selecting workflow: ${workflowName}`);
                    } else {
                        addMessage('error', 'Not connected to server. Please refresh the page.');
                    }
                }
                chatInput.value = '';
                return;
            }
            
            // Create template command
            if (lowerInput.startsWith('/create_template')) {
                const parts = inputText.split(' ');
                if (parts.length >= 3) {
                    const templateType = parts[1];
                    const templateName = parts.slice(2).join(' ');
                    
                    if (templateType === 'mcp' || templateType === 'tool') {
                        if (socket.readyState === WebSocket.OPEN) {
                            socket.send(JSON.stringify({ 
                                command: "create_template", 
                                payload: {
                                    type: templateType,
                                    name: templateName
                                }
                            }));
                            addMessage('system', `Creating ${templateType} template: ${templateName}`);
                        } else {
                            addMessage('error', 'Not connected to server. Please refresh the page.');
                        }
                    } else {
                        addMessage('error', 'Invalid template type. Use "mcp" or "tool".');
                    }
                } else {
                    addMessage('error', 'Usage: /create_template <mcp|tool> <name>');
                }
                chatInput.value = '';
                return;
            }
            
            // Regular message - send to server
            addMessage('you', inputText);
            
            // Send chat messages in the JSON format the backend expects
            if (socket.readyState === WebSocket.OPEN) {
                socket.send(JSON.stringify({ command: "submit", payload: inputText }));
                // Simulate a response if no backend is responding (for testing)
                setTimeout(() => {
                    if (chatMessages.lastElementChild && 
                        chatMessages.lastElementChild.querySelector('.message-from').textContent.includes(localStorage.getItem('userName') || 'User')) {
                        // Only add mock response if no real response came
                        const lastUserMsg = chatMessages.lastElementChild;
                        const nextSibling = lastUserMsg.nextElementSibling;
                        if (!nextSibling || nextSibling.classList.contains('message-you')) {
                            addMessage('agent', 'Processing your request: "' + inputText + '"');
                        }
                    }
                }, 1000);
            } else {
                addMessage('error', 'Not connected to server. Please refresh the page.');
            }
            
            chatInput.value = '';
        }
    }

    sendBtn.addEventListener('click', handleUserInput);
    chatInput.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') handleUserInput();
    });

    addMessage('system', 'Welcome to Neonmachines!');

    // --- POML File Management ---
    const pomlFileList = document.getElementById('poml-file-list');
    if (pomlFileList) {
        // Add some default POML files for testing
        const defaultFiles = [
            'workflow1.poml',
            'security_analyzer.poml', 
            'coding_agent.poml',
            'test_workflow.poml'
        ];
        
        // Clear existing options except the first one
        while (pomlFileList.children.length > 1) {
            pomlFileList.removeChild(pomlFileList.lastChild);
        }
        
        // Add default files to dropdown
        defaultFiles.forEach(file => {
            const option = document.createElement('option');
            option.value = file;
            option.textContent = file;
            pomlFileList.appendChild(option);
        });
        
        // Also try to load from server
        fetch('/api/poml-files')
            .then(response => response.json())
            .then(files => {
                // Add any additional files from server
                files.forEach(file => {
                    if (!defaultFiles.includes(file)) {
                        const option = document.createElement('option');
                        option.value = file;
                        option.textContent = file;
                        pomlFileList.appendChild(option);
                    }
                });
            })
            .catch(error => {
                console.log('Could not load POML files from server, using defaults');
            });

        // Handle file selection
        pomlFileList.addEventListener('change', (e) => {
            const selectedFile = e.target.value;
            if (selectedFile) {
                loadPomlFile(selectedFile);
            }
        });
        
        function loadPomlFile(selectedFile) {
            console.log("Loading POML file:", selectedFile);
            
            // Try to load from localStorage first
            const localContent = localStorage.getItem('poml_' + selectedFile);
            if (localContent) {
                console.log("Found in localStorage, content length:", localContent.length);
                // Check if POML editor tab is active
                const activeTab = document.querySelector('.nav-tab.active');
                const isPomlEditorActive = activeTab && activeTab.getAttribute('data-tab') === 'poml-editor';
                
                // If POML editor is not active, switch to it first
                if (!isPomlEditorActive) {
                    const pomlTab = document.querySelector('.nav-tab[data-tab="poml-editor"]');
                    if (pomlTab) {
                        console.log("Switching to POML editor tab");
                        pomlTab.click();
                        // Wait for the tab to load, then load the file
                        setTimeout(() => {
                            console.log("Trying to load file after tab switch");
                            // Try multiple approaches to load the content
                            // First, try to load using the editor instance
                            if (window.pomlEditorInstance && window.pomlEditorInstance.loadFile) {
                                console.log("Loading via editor instance");
                                window.pomlEditorInstance.loadFile(localContent, selectedFile);
                            } else {
                                // Second, try the global loadPomlFromDropdown function
                                if (typeof window.loadPomlFromDropdown === 'function') {
                                    console.log("Loading via loadPomlFromDropdown function");
                                    window.loadPomlFromDropdown(localContent, selectedFile);
                                } else {
                                    // Third, try to send via WebSocket to the editor
                                    console.log("Loading via WebSocket");
                                    if (window.socket && window.socket.readyState === WebSocket.OPEN) {
                                        window.socket.send(JSON.stringify({
                                            command: "send_poml_to_editor",
                                            payload: {
                                                content: localContent,
                                                file_name: selectedFile
                                            }
                                        }));
                                    }
                                }
                            }
                        }, 500); // Give more time for initialization
                    }
                } else {
                    // POML editor is active, load directly
                    console.log("POML editor already active, loading directly");
                    // Try multiple approaches to load the content
                    if (window.pomlEditorInstance && window.pomlEditorInstance.loadFile) {
                        console.log("Loading via editor instance (direct)");
                        window.pomlEditorInstance.loadFile(localContent, selectedFile);
                    } else if (typeof window.loadPomlFromDropdown === 'function') {
                        console.log("Loading via loadPomlFromDropdown function (direct)");
                        window.loadPomlFromDropdown(localContent, selectedFile);
                    } else {
                        // Send via WebSocket as fallback
                        console.log("Loading via WebSocket (direct)");
                        if (window.socket && window.socket.readyState === WebSocket.OPEN) {
                            window.socket.send(JSON.stringify({
                                command: "send_poml_to_editor",
                                payload: {
                                    content: localContent,
                                    file_name: selectedFile
                                }
                            }));
                        }
                    }
                }
                addMessage('system', `Loaded POML file: ${selectedFile} (from cache)`);
            } else {
                // Create some sample content for testing
                console.log("Creating sample content for:", selectedFile);
                const sampleContent = `name: ${selectedFile.replace('.poml', '')}
description: POML workflow file

agents:
  - name: MainAgent
    type: primary
    task: "Main task to perform"
    max_iterations: 3
    
  - name: HelperAgent
    type: secondary
    task: "Help with the task"
    max_iterations: 2
    
workflow:
  - step: 1
    agent: MainAgent
    action: execute
    
  - step: 2
    agent: HelperAgent
    action: validate`;
                
                // Save to localStorage
                localStorage.setItem('poml_' + selectedFile, sampleContent);
                console.log("Saved sample content to localStorage, length:", sampleContent.length);
                
                // Check if POML editor tab is active
                const activeTab = document.querySelector('.nav-tab.active');
                const isPomlEditorActive = activeTab && activeTab.getAttribute('data-tab') === 'poml-editor';
                
                // If POML editor is not active, switch to it first
                if (!isPomlEditorActive) {
                    const pomlTab = document.querySelector('.nav-tab[data-tab="poml-editor"]');
                    if (pomlTab) {
                        console.log("Switching to POML editor tab for sample content");
                        pomlTab.click();
                        // Wait for the tab to load, then load the file
                        setTimeout(() => {
                            console.log("Trying to load sample content after tab switch");
                            // Try multiple approaches to load the content
                            // First, try to load using the editor instance
                            if (window.pomlEditorInstance && window.pomlEditorInstance.loadFile) {
                                console.log("Loading sample via editor instance");
                                window.pomlEditorInstance.loadFile(sampleContent, selectedFile);
                            } else {
                                // Second, try the global loadPomlFromDropdown function
                                if (typeof window.loadPomlFromDropdown === 'function') {
                                    console.log("Loading sample via loadPomlFromDropdown function");
                                    window.loadPomlFromDropdown(sampleContent, selectedFile);
                                } else {
                                    // Third, try to send via WebSocket to the editor
                                    console.log("Loading sample via WebSocket");
                                    if (window.socket && window.socket.readyState === WebSocket.OPEN) {
                                        window.socket.send(JSON.stringify({
                                            command: "send_poml_to_editor",
                                            payload: {
                                                content: sampleContent,
                                                file_name: selectedFile
                                            }
                                        }));
                                    }
                                }
                            }
                        }, 500); // Give more time for initialization
                    }
                } else {
                    // POML editor is active, load directly
                    console.log("POML editor already active, loading sample directly");
                    // Try multiple approaches to load the content
                    if (window.pomlEditorInstance && window.pomlEditorInstance.loadFile) {
                        console.log("Loading sample via editor instance (direct)");
                        window.pomlEditorInstance.loadFile(sampleContent, selectedFile);
                    } else if (typeof window.loadPomlFromDropdown === 'function') {
                        console.log("Loading sample via loadPomlFromDropdown function (direct)");
                        window.loadPomlFromDropdown(sampleContent, selectedFile);
                    } else {
                        // Send via WebSocket as fallback
                        console.log("Loading sample via WebSocket (direct)");
                        if (window.socket && window.socket.readyState === WebSocket.OPEN) {
                            window.socket.send(JSON.stringify({
                                command: "send_poml_to_editor",
                                payload: {
                                    content: sampleContent,
                                    file_name: selectedFile
                                }
                            }));
                        }
                    }
                }
                
                addMessage('system', `Created sample POML file: ${selectedFile}`);
                
                // Also try to load from server in background
                fetch(`/api/load-poml?file=${encodeURIComponent(selectedFile)}`)
                    .then(response => response.json())
                    .then(data => {
                        if (!data.error && data.content) {
                            // Update localStorage with real content
                            localStorage.setItem('poml_' + selectedFile, data.content);
                            console.log("Updated with server content");
                        }
                    })
                    .catch(error => {
                        console.log('Could not load from server:', error);
                    });
            }
        }
        
        // Make loadPomlFile globally accessible
        window.loadPomlFile = loadPomlFile;
    }

    // Handle the Run button in header
    const runBtn = document.getElementById('run-btn');
    if (runBtn) {
        runBtn.addEventListener('click', () => {
            // Get current tab to determine what to run
            const activeTab = document.querySelector('.nav-tab.active').getAttribute('data-tab');
            if (activeTab === 'poml-editor') {
                // Use the POML editor's run function directly
                if (window.pomlEditorInstance && window.pomlEditorInstance.runPoml) {
                    window.pomlEditorInstance.runPoml();
                } else {
                    // Fallback: get content and send via WebSocket
                    if (window.pomlEditorInstance && window.pomlEditorInstance.getCurrentContent) {
                        const content = window.pomlEditorInstance.getCurrentContent();
                        socket.send(JSON.stringify({ command: "run_poml", payload: content }));
                    } else {
                        socket.send(JSON.stringify({ command: "run_poml", payload: "editor_content" }));
                    }
                }
            } else if (activeTab === 'chat') {
                // Run the last chat command
                const lastMessage = chatMessages.lastElementChild;
                if (lastMessage && lastMessage.querySelector('.message-from')) {
                    const messageFrom = lastMessage.querySelector('.message-from').textContent;
                    if (messageFrom.includes(localStorage.getItem('userName') || 'User')) {
                        const command = lastMessage.querySelector('.message-text').textContent;
                        socket.send(JSON.stringify({ command: "submit", payload: command }));
                    }
                }
            }
        });
    }

    // Handle the Save button in header
    const saveBtn = document.getElementById('save-btn');
    if (saveBtn) {
        saveBtn.addEventListener('click', () => {
            // Get current tab to determine what to save
            const activeTab = document.querySelector('.nav-tab.active').getAttribute('data-tab');
            if (activeTab === 'poml-editor') {
                // Use the POML editor's save function directly
                if (window.pomlEditorInstance && window.pomlEditorInstance.saveFile) {
                    window.pomlEditorInstance.saveFile();
                } else {
                    // Fallback to WebSocket command
                    socket.send(JSON.stringify({ command: "save_poml", payload: "editor_content" }));
                }
            } else if (activeTab === 'graph-editor') {
                // Save graph
                const saveGraphBtn = document.getElementById('save-graph-btn');
                if (saveGraphBtn) {
                    saveGraphBtn.click();
                }
            }
        });
    }
});
