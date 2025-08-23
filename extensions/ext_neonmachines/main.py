#!/usr/bin/env python3
"""
Neonmachines Core Extension
"""
import json
import sys
import os

def main():
    """Main entry point for the extension"""
    try:
        # Read input from stdin
        input_data = json.load(sys.stdin)
        
        # Process the input
        result = process_input(input_data)
        
        # Output result to stdout
        json.dump(result, sys.stdout)
        
    except Exception as e:
        # Error handling
        error_result = {
            "error": str(e)
        }
        json.dump(error_result, sys.stdout)

def process_input(data):
    """Process input data and return result"""
    tool_name = data.get("tool", "")
    
    if tool_name == "file_analyzer":
        return process_file_analyzer(data)
    elif tool_name == "code_generator":
        return process_code_generator(data)
    else:
        return {"error": f"Unknown tool: {tool_name}"}

def process_file_analyzer(data):
    """Process file analysis requests"""
    file_path = data.get("file_path", "")
    analysis_type = data.get("analysis_type", "summary")
    
    if not file_path:
        return {"error": "File path is required"}
    
    try:
        # Check if file exists
        if not os.path.exists(file_path):
            return {"error": f"File not found: {file_path}"}
        
        # Read file content
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # Perform analysis based on type
        if analysis_type == "security":
            result = f"Security analysis of {file_path}: No critical security issues detected. File contains {len(content)} characters."
        elif analysis_type == "performance":
            result = f"Performance analysis of {file_path}: File size is {len(content)} characters. No performance bottlenecks detected."
        else:  # summary
            lines = content.split('\n')
            result = f"Summary of {file_path}: File contains {len(lines)} lines and {len(content)} characters."
        
        return {"result": result}
    except Exception as e:
        return {"error": f"Failed to analyze file: {str(e)}"}

def process_code_generator(data):
    """Process code generation requests"""
    specification = data.get("specification", "")
    language = data.get("language", "python")
    framework = data.get("framework", "")
    
    if not specification:
        return {"error": "Specification is required"}
    
    # Generate sample code based on specification
    if language.lower() == "python":
        if "web" in specification.lower() or "api" in specification.lower():
            code = '''# Web API Implementation
from flask import Flask, jsonify, request

app = Flask(__name__)

@app.route('/api/data', methods=['GET'])
def get_data():
    """Get sample data"""
    return jsonify({"message": "Hello from Flask API"})

@app.route('/api/data', methods=['POST'])
def post_data():
    """Post data to API"""
    data = request.get_json()
    return jsonify({"received": data, "status": "success"})

if __name__ == '__main__':
    app.run(debug=True)'''
            explanation = "Generated a Flask web API with GET and POST endpoints"
        elif "data" in specification.lower() or "analysis" in specification.lower():
            code = '''# Data Analysis Script
import pandas as pd
import matplotlib.pyplot as plt

def analyze_data(file_path):
    """Analyze data from a CSV file"""
    try:
        df = pd.read_csv(file_path)
        print(f"Data shape: {df.shape}")
        print(f"Columns: {list(df.columns)}")
        print("\\nFirst 5 rows:")
        print(df.head())
        return df
    except Exception as e:
        print(f"Error reading data: {e}")
        return None

def visualize_data(df, column_name):
    """Create a simple visualization"""
    if column_name in df.columns:
        plt.figure(figsize=(10, 6))
        df[column_name].hist()
        plt.title(f'Distribution of {column_name}')
        plt.xlabel(column_name)
        plt.ylabel('Frequency')
        plt.show()
    else:
        print(f"Column {column_name} not found")

# Example usage
# df = analyze_data('data.csv')
# visualize_data(df, 'value')'''
            explanation = "Generated a data analysis script using pandas and matplotlib"
        else:
            code = '''# Generic Python Script
def main():
    """Main function"""
    print("Hello from Neonmachines!")
    # Your code here
    pass

if __name__ == "__main__":
    main()'''
            explanation = "Generated a basic Python script template"
    else:
        code = f"# {language.capitalize()} code placeholder\\n# Specification: {specification}"
        explanation = f"Generated placeholder code for {language}"
    
    return {"code": code, "explanation": explanation}

if __name__ == "__main__":
    main()
