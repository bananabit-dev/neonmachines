import * as monaco from 'monaco-editor';
import * as d3 from 'd3';

class PomlEditor {
    constructor(selector) {
        this.container = document.querySelector(selector);
        this.editor = monaco.editor.create(this.container, {
            value: '',
            language: 'yaml',
            theme: 'vs-dark'
        });
        this.setupEventListeners();
    }

    setupEventListeners() {
        d3.select("#validate-btn").on("click", () => this.validate());
        d3.select("#save-file-btn").on("click", () => this.saveFile());
        d3.select("#open-file-btn").on("click", () => this.loadFile());
    }

    getContent() {
        return this.editor.getValue();
    }

    setContent(content) {
        this.editor.setValue(content);
    }

    async validate() {
        const content = this.getContent();
        const outputContent = document.querySelector("#poml-output-content");
        
        outputContent.innerText = "Validating...";

        try {
            const response = await fetch('/api/poml/validate', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ poml: content })
            });

            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }

            const result = await response.json();

            if (result.status === 'ok') {
                outputContent.innerText = "POML is valid.";
            } else {
                outputContent.innerText = `Validation failed:
${result.message}`;
            }
        } catch (error) {
            outputContent.innerText = `An error occurred: ${error.message}`;
        }
    }

    saveFile() {
        const content = this.getContent();
        const blob = new Blob([content], { type: 'text/yaml;charset=utf-8' });
        const link = document.createElement('a');
        link.href = URL.createObjectURL(blob);
        link.download = 'poml.yaml';
        link.click();
    }

    loadFile() {
        const input = document.createElement('input');
        input.type = 'file';
        input.onchange = e => {
            const file = e.target.files[0];
            const reader = new FileReader();
            reader.onload = readerEvent => {
                const content = readerEvent.target.result;
                this.setContent(content);
            }
            reader.readAsText(file);
        }
        input.click();
    }
}

document.addEventListener('DOMContentLoaded', () => {
    if (document.querySelector('#poml-editor .code-editor')) {
        new PomlEditor('#poml-editor .code-editor');
    }
});
