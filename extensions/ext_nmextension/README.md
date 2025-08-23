# NMExtension Template Generator

This is an NMExtension for Neonmachines that generates project templates.

## Description

The NMExtension Template Generator provides a way to quickly create project templates for different types of applications within the Neonmachines ecosystem.

## Tools

### template_generator
Generates project templates for various application types
- **Parameters**: 
  - `template_type` (required): Type of template to generate (web, cli, api, library)
  - `project_name` (optional): Name of the project (default: "my_project")
  - `output_dir` (optional): Output directory for the template (default: current directory)

## Capabilities

- Model Control: ✗
- Tool Integration: ✓
- File Operations: ✓
- System Access: ✓

## Installation

The extension is automatically loaded when Neonmachines starts. Ensure the extension directory is in the extensions path.

## Usage

This extension can be used to generate starter templates for:
- Web applications
- CLI tools
- API services
- Library modules

## Requirements

- bash
- jq (for JSON parsing)

Install jq with:
```bash
# Ubuntu/Debian
sudo apt-get install jq

# macOS
brew install jq

# CentOS/RHEL
sudo yum install jq
```
