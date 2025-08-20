function initializeGraphEditor() {
    if (!window.socket) {
        console.error("Socket not found");
        return;
    }

    const socket = window.socket;
    const addNodeBtn = document.getElementById('add-node-btn');
    const connectBtn = document.getElementById('connect-btn');
    const deleteBtn = document.getElementById('delete-btn');
    const loadGraphBtn = document.getElementById('load-graph-btn');
    const propertiesPanel = document.getElementById('node-properties-panel');

    let nodes = [];
    let connections = [];
    let selectedNodes = [];
    let isConnecting = false;

    // Function to send commands to the server
    const sendCommand = (command, payload) => {
        socket.send(JSON.stringify({ command, payload }));
    };

    addNodeBtn.addEventListener('click', () => {
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

    connectBtn.addEventListener('click', () => {
        isConnecting = true;
        selectedNodes = [];
    });

    deleteBtn.addEventListener('click', () => {
        if (selectedNodes.length > 0) {
            nodes = nodes.filter(n => !selectedNodes.includes(n.id));
            connections = connections.filter(c => !selectedNodes.includes(c.source) && !selectedNodes.includes(c.target));
            selectedNodes = [];
            renderNodeProperties(null);
            renderGraph();
        }
    });

    loadGraphBtn.addEventListener('click', () => {
        fetch('static/graph.json')
            .then(response => response.json())
            .then(data => {
                nodes = data.nodes;
                connections = data.connections;
                renderGraph();
            })
            .catch(error => {
                console.error('Error loading graph:', error);
            });
    });

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
                <input type="text" value="${node.on_success || ''}" readonly>
            </div>
            <div class="property-item">
                <label>On Failure:</label>
                <input type="text" value="${node.on_failure || ''}" readonly>
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
                line.setAttribute('stroke', connection.type === 'success' ? 'green' : 'red');
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
            
            const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            text.setAttribute('x', node.x);
            text.setAttribute('y', node.y);
            text.setAttribute('text-anchor', 'middle');
            text.setAttribute('dy', '.3em');
            text.textContent = node.type.substring(0, 1);
            
            nodeElement.appendChild(circle);
            nodeElement.appendChild(text);

            nodeElement.addEventListener('click', (e) => {
                e.stopPropagation(); // Prevent canvas click event from firing
                handleNodeClick(node);
            });
            nodesGroup.appendChild(nodeElement);
        });
    }

    renderGraph();
}

initializeGraphEditor();
