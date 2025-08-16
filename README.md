# Neonmachines

A graph-based AI Orchestration framework that uses **POML** in the background for prepared AI calls so context is not wasted.  
This should require fewer AI calls and provide better output.  
It also makes use of **tools** (built-in, custom, and MCP servers).

---

## Features

- Graph-based multi-agent orchestration
- POML prompt files for structured context
- Built-in tools:
  - `pwd`, `cd`, `ls`, `grep`, `mkdir`, `touch`
- Custom tools via `.nmextension` files
- Ignore files with `.nmignore`
- MCP server integration via `.nmmcpextension`
- Configurable validator agents with success/failure routing
- Traversal limits to prevent infinite loops

---

## Requirements

- Python with POML:

```bash
pip install poml
```

Usage:

```bash
python -m poml -f {{file}}
```

- Rust dependencies (in `Cargo.toml`):

```toml
filetime = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

---

## Extensions

- **Custom tools**:  
  Create `.nmextension` files with JSON describing tools.  
  Example:

  ```json
  [
    {
      "tool_type": "function",
      "function": {
        "name": "count_lines",
        "description": "Count lines in a file",
        "parameters": {
          "type": "object",
          "properties": {
            "path": { "type": "string", "description": "File to count lines in" }
          },
          "required": ["path"]
        }
      }
    }
  ]
  ```

- **MCP servers**:  
  Create `.nmmcpextension` files to describe external MCP endpoints.  
  Example:

  ```json
  [
    {
      "tool_type": "function",
      "function": {
        "name": "mcp_weather",
        "description": "Fetch weather from MCP server",
        "parameters": {
          "type": "object",
          "properties": {
            "location": { "type": "string", "description": "City name" }
          },
          "required": ["location"]
        }
      }
    }
  ]
  ```

- **Ignore files**:  
  Add patterns to `.nmignore` to skip files (like `.gitignore`).

---

## Example Workflow

```bash
/neonmachines
  ├── config.nm
  ├── prompts/
  │   ├── system.poml
  │   └── user.poml
  ├── tools/
  │   ├── mytools.nmextension
  │   └── weather.nmmcpextension
  └── .nmignore
```

---

## Roadmap

- Add `cat` tool for file reading
- Add streaming tool outputs
- Add UI for managing extensions