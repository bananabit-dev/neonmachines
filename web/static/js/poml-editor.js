// Global variable to store editor instance
let globalPomlEditor = null;

function initializePomlEditor() {
    console.log("Initializing POML Editor...");
    
    if (!window.socket) {
        console.error("Socket not found");
        return;
    }

    const socket = window.socket;
    const newFileBtn = document.getElementById('new-file-btn');
    const saveFileBtn = document.getElementById('save-file-btn');
    const validateBtn = document.getElementById('validate-btn');
    const runPomlBtn = document.getElementById('run-poml-btn');
    const pomlEditor = document.getElementById('poml-editor');
    const pomlOutput = document.getElementById('poml-output-content');
    const currentFileNameElement = document.getElementById('current-file-name');

    let currentFileName = null;
    let hasUnsavedChanges = false;

    // Initialize CodeMirror editor with YAML mode
    const editor = CodeMirror.fromTextArea(pomlEditor, {
        lineNumbers: true,
        mode: 'yaml',
        theme: 'monokai',
        lineWrapping: true,
        autofocus: true,
        extraKeys: {
            "Ctrl-S": function() {
                saveFile();
                return false; // Prevent browser save dialog
            },
            "Ctrl-Enter": function() {
                runPoml();
                return false;
            }
        }
    });
    
    // Store editor globally for access from other scripts
    globalPomlEditor = editor;
    
    // Set the size explicitly after initialization
    setTimeout(() => {
        editor.setSize(null, "500px");
        editor.refresh(); // Force refresh to apply size
    }, 100);

    // Track changes
    editor.on('change', (cm, change) => {
        // Ignore programmatic changes
        if (change.origin !== 'setValue') {
            hasUnsavedChanges = true;
            updateFileNameDisplay();
        }
    });

    // Function to update the file name display
    function updateFileNameDisplay() {
        if (currentFileName) {
            currentFileNameElement.textContent = currentFileName + (hasUnsavedChanges ? ' *' : '');
        } else {
            currentFileNameElement.textContent = 'New File' + (hasUnsavedChanges ? ' *' : '');
        }
        
        // Also update the dropdown in the header if it exists
        const pomlFileList = document.getElementById('poml-file-list');
        if (pomlFileList && currentFileName) {
            pomlFileList.value = currentFileName;
        }
    }
    
    // Function to load a file (exposed for external use)
    function loadFile(content, fileName) {
        console.log("Loading file in loadFile function:", fileName);
        console.log("Content length:", content ? content.length : 0);
        editor.setValue(content || '');
        currentFileName = fileName;
        hasUnsavedChanges = false;
        updateFileNameDisplay();
        pomlOutput.innerHTML = '<div class="success">Loaded file: ' + fileName + '</div>';
        
        // Force refresh and focus
        setTimeout(() => {
            editor.refresh();
            editor.focus();
        }, 100);
    }

    // Function to save file
    function saveFile() {
        const content = editor.getValue();
        
        if (!currentFileName) {
            // Prompt for filename if it's a new file
            const fileName = prompt('Enter filename (with .poml extension):', 'untitled.poml');
            if (!fileName) return;
            currentFileName = fileName;
        }
        
        // Save to localStorage
        localStorage.setItem('poml_' + currentFileName, content);
        localStorage.setItem('poml_current_file', currentFileName);
        
        // Send to server if connected
        if (socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify({ 
                command: "save_poml", 
                payload: {
                    content: content,
                    filename: currentFileName
                }
            }));
        }
        
        hasUnsavedChanges = false;
        updateFileNameDisplay();
        
        // Show success message
        pomlOutput.innerHTML = '<div class="success">File saved: ' + currentFileName + '</div>';
        setTimeout(() => {
            pomlOutput.innerHTML = 'Ready to run POML commands...';
        }, 3000);
    }

    // Function to run POML
    function runPoml() {
        const content = editor.getValue();
        pomlOutput.innerHTML = '<p>Running POML...</p>';
        
        if (socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify({ 
                command: "run_poml", 
                payload: content 
            }));
        } else {
            pomlOutput.innerHTML = '<div class="error">Not connected to server</div>';
        }
    }

    // Function to validate POML
    function validatePoml() {
        const content = editor.getValue();
        pomlOutput.innerHTML = '<p>Validating POML syntax...</p>';
        
        // Basic validation
        try {
            // Check if it's valid YAML-like syntax
            if (!content.trim()) {
                pomlOutput.innerHTML = '<div class="error">Error: Empty file</div>';
                return;
            }
            
            // Check for basic POML structure
            if (!content.includes('name:') && !content.includes('agents:')) {
                pomlOutput.innerHTML = '<div class="error">Warning: Missing required POML fields (name, agents)</div>';
                return;
            }
            
            pomlOutput.innerHTML = '<div class="success">Validation passed!</div>';
            
            // Send to server for deeper validation if connected
            if (socket.readyState === WebSocket.OPEN) {
                socket.send(JSON.stringify({ 
                    command: "validate_poml", 
                    payload: content 
                }));
            }
        } catch (e) {
            pomlOutput.innerHTML = '<div class="error">Validation error: ' + e.message + '</div>';
        }
    }

    // Listen for messages from WebSocket
    const handleMessage = (event) => {
        try {
            const response = JSON.parse(event.data);
            console.log('POML Editor received message:', response);
            
            // Handle file loading from dropdown
            if (response.command === 'send_poml_to_editor' && response.payload) {
                const content = response.payload.content || '';
                const fileName = response.payload.file_name || response.payload.filename || null;
                console.log('Loading content from WebSocket:', fileName);
                loadFile(content, fileName);
                return;
            }
            
            // Handle save confirmation
            if (response.status === 'poml_saved' || response.command === 'poml_saved') {
                pomlOutput.innerHTML = '<div class="success">File saved successfully!</div>';
                hasUnsavedChanges = false;
                updateFileNameDisplay();
                return;
            }
            
            // Handle validation results
            if (response.status === 'poml_validation_result') {
                if (response.data && response.data.valid) {
                    pomlOutput.innerHTML = '<div class="success">Validation passed!</div>';
                } else {
                    pomlOutput.innerHTML = '<div class="error">Validation failed: ' + (response.data?.error || 'Unknown error') + '</div>';
                }
                return;
            }
            
            // Handle run results
            if (response.status === 'poml_run_result' || response.status === 'result') {
                const outputContent = response.data || '';
                if (typeof outputContent === 'string' && outputContent.trim().startsWith('<')) {
                    pomlOutput.innerHTML = outputContent;
                } else {
                    pomlOutput.innerHTML = '<pre>' + outputContent + '</pre>';
                }
                return;
            }
            
            // Handle errors
            if (response.status === 'error') {
                pomlOutput.innerHTML = '<div class="error">Error: ' + (response.data || 'Unknown error') + '</div>';
                return;
            }
        } catch (e) {
            // Not a JSON message, ignore
            console.log('Non-JSON message in POML editor:', event.data);
        }
    };
    
    // Add WebSocket listener
    console.log("Adding WebSocket listener for POML editor");
    socket.addEventListener('message', handleMessage);

    // Button event listeners
    newFileBtn.addEventListener('click', () => {
        if (hasUnsavedChanges) {
            if (!confirm('You have unsaved changes. Create new file anyway?')) {
                return;
            }
        }
        editor.setValue('');
        currentFileName = null;
        hasUnsavedChanges = false;
        updateFileNameDisplay();
        pomlOutput.innerHTML = 'Ready to run POML commands...';
    });

    saveFileBtn.addEventListener('click', saveFile);
    validateBtn.addEventListener('click', validatePoml);
    runPomlBtn.addEventListener('click', runPoml);

    // Load last edited file from localStorage if available
    const lastFile = localStorage.getItem('poml_current_file');
    if (lastFile) {
        const content = localStorage.getItem('poml_' + lastFile);
        if (content) {
            loadFile(content, lastFile);
        }
    } else {
        // Initialize with a sample POML if no previous file
        const samplePoml = `name: Example Workflow
description: A sample POML workflow

agents:
  - name: PrimaryAgent
    type: primary
    task: "Perform the main task"
    max_iterations: 3
    
  - name: SecondaryAgent
    type: secondary
    task: "Assist with validation"
    max_iterations: 2
    
workflow:
  - step: 1
    agent: PrimaryAgent
    action: execute
    input: "Start the process"
    
  - step: 2
    agent: SecondaryAgent
    action: validate
    input: "Check the results"`;
        
        editor.setValue(samplePoml);
        hasUnsavedChanges = true;
        updateFileNameDisplay();
    }
    
    // Expose the editor instance and methods globally
    window.pomlEditorInstance = {
        editor: editor,
        loadFile: loadFile,
        saveFile: saveFile,
        runPoml: runPoml,
        validatePoml: validatePoml,
        getCurrentContent: () => editor.getValue(),
        setContent: (content) => editor.setValue(content),
        getCurrentFileName: () => currentFileName
    };
    
    // Also make loadFile directly accessible
    window.loadPomlFile = loadFile;
    
    console.log("POML Editor initialized successfully");
}

// Function to handle external file loading (can be called from main.js)
window.loadPomlFromDropdown = function(content, fileName) {
    console.log("loadPomlFromDropdown called:", fileName);
    console.log("Content length in loadPomlFromDropdown:", content ? content.length : 0);
    if (window.pomlEditorInstance && window.pomlEditorInstance.loadFile) {
        console.log("Using pomlEditorInstance.loadFile");
        window.pomlEditorInstance.loadFile(content, fileName);
    } else if (globalPomlEditor) {
        // Fallback: directly set content if editor exists
        console.log("Using globalPomlEditor directly");
        globalPomlEditor.setValue(content || '');
        const currentFileNameElement = document.getElementById('current-file-name');
        if (currentFileNameElement) {
            currentFileNameElement.textContent = fileName || 'New File';
        }
    } else {
        console.error("POML Editor not initialized yet");
    }
};
