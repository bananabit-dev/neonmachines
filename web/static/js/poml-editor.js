function initializePomlEditor() {
    if (!window.socket) {
        console.error("Socket not found");
        return;
    }

    const socket = window.socket;
    const newFileBtn = document.getElementById('new-file-btn');
    const openFileBtn = document.getElementById('open-file-btn');
    const saveFileBtn = document.getElementById('save-file-btn');
    const formatBtn = document.getElementById('format-btn');
    const validateBtn = document.getElementById('validate-btn');
    const runPomlBtn = document.getElementById('run-poml-btn');
    const pomlEditor = document.getElementById('poml-editor');
    const pomlOutput = document.getElementById('poml-output-content');

    const editor = CodeMirror.fromTextArea(pomlEditor, {
        lineNumbers: true,
        mode: 'yaml',
        theme: 'default'
    });

    newFileBtn.addEventListener('click', () => {
        editor.setValue('');
    });

    openFileBtn.addEventListener('click', () => {
        // This would require a file open dialog, which is complex.
        // For now, we'll just log a message.
        console.log("Open file functionality not yet implemented.");
    });

    saveFileBtn.addEventListener('click', () => {
        const content = editor.getValue();
        socket.send(JSON.stringify({ command: "save_poml", payload: content }));
    });

    formatBtn.addEventListener('click', () => {
        // This would require a YAML formatter.
        // For now, we'll just log a message.
        console.log("Format functionality not yet implemented.");
    });

    validateBtn.addEventListener('click', () => {
        const content = editor.getValue();
        socket.send(JSON.stringify({ command: "validate_poml", payload: content }));
    });

    runPomlBtn.addEventListener('click', () => {
        const content = editor.getValue();
        socket.send(JSON.stringify({ command: "run_poml", payload: content }));
    });
}

initializePomlEditor();
