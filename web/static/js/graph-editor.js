function initializeGraphEditor() {
    console.log("Initializing graph editor...");
    if (!window.socket) {
        console.error("Socket not found");
        return;
    }
    console.log("Socket found, proceeding with initialization");

    const socket = window.socket;
    const addNodeBtn = document.getElementById('add-node-btn');
    const connectBtn = document.getElementById('connect-btn');
    const deleteBtn = document.getElementById('delete-btn');
    const loadGraphBtn = document.getElementById('load-graph-btn');
    const propertiesPanel = document.getElementById('node-properties-panel');
    
    console.log("DOM elements found:", {addNodeBtn, connectBtn, deleteBtn, loadGraphBtn, propertiesPanel});

    let nodes = [];
    let connections = [];
    let selectedNodes = [];
    let isConnecting = false;

    // Function to send commands to the server
    const sendCommand = (command, payload) => {
        socket.send(JSON.stringify({ command, payload }));
    };

    if (addNodeBtn) {
        addNodeBtn.addEventListener('click', () => {
            console.log("Add node button clicked");
            const node = {
                id: nodes.length,
                x: 100 + nodes.length * 50,
                y: 100,
                type: 'Agent',
                files: '',
                max_iterations: 3,
                on_success: null,
                on_failure: null,
            };
            nodes.push(node);
            sendCommand('add_node', node);
            renderGraph();
        });
    }

    if (connectBtn) {
        connectBtn.addEventListener('click', () => {
            console.log("Connect button clicked");
            isConnecting = true;
            selectedNodes = [];
        });
    }

    if (deleteBtn) {
        deleteBtn.addEventListener('click', () => {
            console.log("Delete button clicked");
            if (selectedNodes.length > 0) {
                nodes = nodes.filter(n => !selectedNodes.includes(n.id));
                connections = connections.filter(c => !selectedNodes.includes(c.source) && !selectedNodes.includes(c.target));
                selectedNodes = [];
                renderNodeProperties(null);
                renderGraph();
            }
        });
    }

    // Add save button functionality
    const saveGraphBtn = document.getElementById('save-graph-btn');
    if (saveGraphBtn) {
        saveGraphBtn.addEventListener('click', () => {
            console.log("Save graph button clicked");
            const graphData = {
                nodes: nodes,
                connections: connections
            };
            // Send save command to server
            sendCommand('save_graph', graphData);
            
            // Also save to localStorage as backup
            localStorage.setItem('graphData', JSON.stringify(graphData));
            
            // Show confirmation
            const statusText = document.getElementById('status-text');
            if (statusText) {
                statusText.textContent = 'Graph saved successfully';
                setTimeout(() => {
                    statusText.textContent = 'Ready';
                }, 2000);
            }
        });
    }

    if (loadGraphBtn) {
        loadGraphBtn.addEventListener('click', () => {
            console.log("Load graph button clicked");
            
            // First try to load from localStorage
            const savedGraph = localStorage.getItem('graphData');
            if (savedGraph) {
                try {
                    const data = JSON.parse(savedGraph);
                    nodes = data.nodes || [];
                    connections = data.connections || [];
                    renderGraph();
                    console.log("Graph loaded from localStorage");
                    return;
                } catch (e) {
                    console.error('Error loading from localStorage:', e);
                }
            }
            
            // Fall back to loading from file
            fetch('static/graph.json')
                .then(response => response.json())
                .then(data => {
                    nodes = data.nodes || [];
                    connections = data.connections || [];
                    renderGraph();
                    console.log("Graph loaded from file");
                })
                .catch(error => {
                    console.error('Error loading graph:', error);
                    alert('Could not load graph. Please check if graph.json exists.');
                });
        });
    }

    function handleNodeClick(node) {
        if (isConnecting) {
            selectedNodes.push(node.id);
            if (selectedNodes.length === 2) {
                const type = prompt('Enter connection type (success/failure):');
                if (type === 'success' || type === 'failure') {
                    const sourceNode = nodes.find(n => n.id === selectedNodes[0]);
                    if (type === 'success') {
                        sourceNode.on_success = selectedNodes[1];
                    } else {
                        sourceNode.on_failure = selectedNodes[1];
                    }
                    connections.push({
                        source: selectedNodes[0],
                        target: selectedNodes[1],
                        type: type
                    });
                } else {
                    alert('Invalid connection type. Please enter "success" or "failure".');
                }
                isConnecting = false;
                selectedNodes = [];
            }
        } else {
            selectedNodes = [node.id];
            renderNodeProperties(node);
        }
        renderGraph();
    }

    function renderNodeProperties(node) {
        propertiesPanel.innerHTML = ''; // Clear panel

        if (!node) {
            propertiesPanel.innerHTML = '<p class="placeholder-text">Select a node to edit its properties</p>';
            return;
        }

        // Find connected nodes for success and failure routes
        let onSuccessNode = null;
        let onFailureNode = null;
        
        connections.forEach(conn => {
            if (conn.source === node.id) {
                if (conn.type === 'success') {
                    onSuccessNode = nodes.find(n => n.id === conn.target);
                } else if (conn.type === 'failure') {
                    onFailureNode = nodes.find(n => n.id === conn.target);
                }
            }
        });

        const propertiesHTML = `
            <div class="property-item">
                <label>ID:</label>
                <input type="text" value="${node.id}" readonly>
            </div>
            <div class="property-item">
                <label>Type:</label>
                <select id="node-type-input">
                    <option value="Agent" ${node.type === 'Agent' ? 'selected' : ''}>Agent</option>
                    <option value="Validator" ${node.type === 'Validator' ? 'selected' : ''}>Validator</option>
                </select>
            </div>
            <div class="property-item">
                <label>Files:</label>
                <input type="text" id="node-files-input" value="${node.files}">
            </div>
            <div class="property-item">
                <label>Max Iterations:</label>
                <input type="number" id="node-max-iterations-input" value="${node.max_iterations}">
            </div>
            <div class="property-item">
                <label>On Success:</label>
                <input type="text" value="${onSuccessNode ? onSuccessNode.id : ''}" readonly>
            </div>
            <div class="property-item">
                <label>On Failure:</label>
                <input type="text" value="${onFailureNode ? onFailureNode.id : ''}" readonly>
            </div>
        `;
        propertiesPanel.innerHTML = propertiesHTML;

        // Add event listeners to update node data
        document.getElementById('node-type-input').addEventListener('change', (e) => {
            node.type = e.target.value;
        });
        document.getElementById('node-files-input').addEventListener('change', (e) => {
            node.files = e.target.value;
        });
        document.getElementById('node-max-iterations-input').addEventListener('change', (e) => {
            node.max_iterations = parseInt(e.target.value, 10);
        });
    }

    function renderGraph() {
        const nodesGroup = document.getElementById('graph-nodes');
        const connectionsGroup = document.getElementById('graph-connections');

        nodesGroup.innerHTML = '';
        connectionsGroup.innerHTML = '';

        connections.forEach(connection => {
            const sourceNode = nodes.find(n => n.id === connection.source);
            const targetNode = nodes.find(n => n.id === connection.target);

            if (sourceNode && targetNode) {
                const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
                line.setAttribute('x1', sourceNode.x);
                line.setAttribute('y1', sourceNode.y);
                line.setAttribute('x2', targetNode.x);
                line.setAttribute('y2', targetNode.y);
                line.setAttribute('stroke', connection.type === 'success' ? '#0066cc' : '#dc3545');
                line.setAttribute('stroke-width', 2);
                line.setAttribute('marker-end', `url(#arrowhead-${connection.type})`);
                connectionsGroup.appendChild(line);
            }
        });

        nodes.forEach(node => {
            const nodeElement = document.createElementNS('http://www.w3.org/2000/svg', 'g');
            const circle = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
            circle.setAttribute('cx', node.x);
            circle.setAttribute('cy', node.y);
            circle.setAttribute('r', 20);
            circle.setAttribute('class', selectedNodes.includes(node.id) ? 'node selected' : 'node');
            circle.setAttribute('fill', '#0066cc'); // Professional blue for nodes
            circle.setAttribute('stroke', selectedNodes.includes(node.id) ? '#0056b3' : '#0066cc'); // Darker blue stroke for selected
            circle.setAttribute('stroke-width', '2px');
            
            const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            text.setAttribute('x', node.x);
            text.setAttribute('y', node.y);
            text.setAttribute('text-anchor', 'middle');
            text.setAttribute('dy', '.3em');
            text.setAttribute('fill', '#ffffff'); // White text for contrast
            text.setAttribute('stroke', 'none');
            text.textContent = node.type.substring(0, 1);
            
            nodeElement.appendChild(circle);
            nodeElement.appendChild(text);

            // Add drag functionality
            let isDragging = false;
            let startX, startY, startNodeX, startNodeY;

            nodeElement.addEventListener('mousedown', (e) => {
                // Only start dragging if not connecting nodes
                if (!isConnecting) {
                    isDragging = true;
                    startX = e.clientX;
                    startY = e.clientY;
                    startNodeX = node.x;
                    startNodeY = node.y;
                    e.stopPropagation();
                }
            });

            document.addEventListener('mousemove', (e) => {
                if (isDragging) {
                    const dx = e.clientX - startX;
                    const dy = e.clientY - startY;
                    node.x = startNodeX + dx;
                    node.y = startNodeY + dy;
                    circle.setAttribute('cx', node.x);
                    circle.setAttribute('cy', node.y);
                    text.setAttribute('x', node.x);
                    text.setAttribute('y', node.y);
                    
                    // Update connections
                    renderGraph();
                }
            });

            document.addEventListener('mouseup', () => {
                isDragging = false;
            });

            nodeElement.addEventListener('click', (e) => {
                e.stopPropagation(); // Prevent canvas click event from firing
                handleNodeClick(node);
            });
            nodesGroup.appendChild(nodeElement);
        });
    }

    renderGraph();
}

//initializeGraphEditor();
