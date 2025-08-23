#!/bin/bash
# NMExtension Template Generator

# Read input from stdin
input=$(cat)

# Parse the input JSON
tool=$(echo "$input" | jq -r '.tool // empty')
template_type=$(echo "$input" | jq -r '.template_type // "web"')
project_name=$(echo "$input" | jq -r '.project_name // "my_project"')
output_dir=$(echo "$input" | jq -r '.output_dir // "."')

# Function to generate web template
generate_web_template() {
    local project_dir="$output_dir/$project_name"
    mkdir -p "$project_dir"
    
    # Create basic web project structure
    mkdir -p "$project_dir/src"
    mkdir -p "$project_dir/static"
    mkdir -p "$project_dir/templates"
    
    # Create index.html
    cat > "$project_dir/index.html" << EOF
<!DOCTYPE html>
<html>
<head>
    <title>$project_name</title>
</head>
<body>
    <h1>Welcome to $project_name</h1>
    <p>This is a generated web template.</p>
</body>
</html>
EOF
    
    # Create a simple CSS file
    cat > "$project_dir/static/style.css" << EOF
body {
    font-family: Arial, sans-serif;
    margin: 40px;
    background-color: #f5f5f5;
}

h1 {
    color: #333;
}
EOF
    
    echo "Web template generated successfully in $project_dir"
}

# Function to generate CLI template
generate_cli_template() {
    local project_dir="$output_dir/$project_name"
    mkdir -p "$project_dir"
    
    # Create main script
    cat > "$project_dir/main.sh" << EOF
#!/bin/bash
# $project_name CLI Tool

echo "Welcome to $project_name CLI tool"

# Add your CLI logic here
case "\$1" in
    help)
        echo "Usage: \$0 [command]"
        echo "Commands:"
        echo "  help    - Show this help"
        echo "  version - Show version"
        ;;
    version)
        echo "$project_name version 1.0.0"
        ;;
    *)
        echo "Hello from $project_name!"
        ;;
esac
EOF
    
    chmod +x "$project_dir/main.sh"
    
    echo "CLI template generated successfully in $project_dir"
}

# Function to generate API template
generate_api_template() {
    local project_dir="$output_dir/$project_name"
    mkdir -p "$project_dir"
    
    # Create API server script
    cat > "$project_dir/server.py" << EOF
#!/usr/bin/env python3
"""
$project_name API Server
"""
from http.server import HTTPServer, BaseHTTPRequestHandler
import json

class APIServer(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/api/health':
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            response = {'status': 'healthy', 'service': '$project_name'}
            self.wfile.write(json.dumps(response).encode())
        else:
            self.send_response(404)
            self.end_headers()
            self.wfile.write(b'Not Found')

    def do_POST(self):
        if self.path == '/api/data':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            response = {'received': post_data.decode(), 'status': 'success'}
            self.wfile.write(json.dumps(response).encode())
        else:
            self.send_response(404)
            self.end_headers()
            self.wfile.write(b'Not Found')

if __name__ == '__main__':
    server = HTTPServer(('localhost', 8000), APIServer)
    print(f"Starting $project_name API server on http://localhost:8000")
    server.serve_forever()
EOF
    
    echo "API template generated successfully in $project_dir"
}

# Function to generate library template
generate_library_template() {
    local project_dir="$output_dir/$project_name"
    mkdir -p "$project_dir/src"
    
    # Create library module
    cat > "$project_dir/src/lib.py" << EOF
"""
$project_name Library
"""

def hello_world():
    """Return a greeting message"""
    return "Hello from $project_name!"

def add_numbers(a, b):
    """Add two numbers together"""
    return a + b

def process_data(data):
    """Process data in some way"""
    if isinstance(data, list):
        return {"count": len(data), "items": data}
    elif isinstance(data, dict):
        return {"keys": list(data.keys()), "data": data}
    else:
        return {"value": str(data)}

if __name__ == "__main__":
    print(hello_world())
EOF
    
    # Create setup.py for the library
    cat > "$project_dir/setup.py" << EOF
from setuptools import setup, find_packages

setup(
    name="$project_name",
    version="1.0.0",
    packages=find_packages(),
    install_requires=[],
    author="Your Name",
    description="$project_name library",
    python_requires=">=3.6",
)
EOF
    
    echo "Library template generated successfully in $project_dir"
}

# Main execution logic
case "$tool" in
    template_generator)
        case "$template_type" in
            web)
                generate_web_template
                ;;
            cli)
                generate_cli_template
                ;;
            api)
                generate_api_template
                ;;
            library)
                generate_library_template
                ;;
            *)
                echo "Error: Unknown template type: $template_type" >&2
                exit 1
                ;;
        esac
        ;;
    *)
        echo "Error: Unknown tool: $tool" >&2
        exit 1
        ;;
esac
