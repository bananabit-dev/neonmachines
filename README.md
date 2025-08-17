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
- Interactive chat mode with template variable support

---

## Workflow Routing System

The workflow system supports dynamic routing between agents based on validation results:

### Routing Configuration

In your `config.nm` file, you can configure routing for each agent using:

- `on_success`: The node to route to when an agent/validation succeeds
- `on_failure`: The node to route to when a validation fails

### Special Route Values

- **`-1`**: End the workflow (no more nodes to execute)
- **`0` to `N`**: Route to a specific node (0-based indexing)
  - `0` = agent_1
  - `1` = agent_2
  - etc.

### Example Configuration

```
agent_1: Agent
on_success:1      # Go to agent_2 on success
on_failure:0      # Retry agent_1 on failure

agent_2: ValidatorAgent
on_success:-1     # End workflow on validation success
on_failure:0      # Go back to agent_1 on validation failure
```

### ValidatorAgent Behavior

The ValidatorAgent uses **JSON structure validation** (similar to Pydantic) to determine success/failure:

#### Validation Logic:

1. **Explicit Validation Result**: If response contains a `valid` field:
```json
{
  "valid": true,
  "errors": [],
  "data": { ... }
}
```
The `valid` field value determines the validation result.

2. **Any Valid JSON Structure**: Without explicit `valid` field:
```json
{
  "name": "example",
  "items": ["a", "b", "c"],
  "count": 42
}
```
Any well-formed JSON is considered **valid** by default.

3. **Invalid or Missing JSON**:
```
This is plain text without JSON
```
Responses without valid JSON structure are considered **invalid**.

#### How It Works:
- Attempts to parse the entire response as JSON
- If that fails, extracts embedded JSON objects `{...}` or arrays `[...]`
- Validates the extracted JSON structure
- Routes based on validation success or failure

#### Benefits:
- **Generic**: Works with any JSON structure defined by user POML files
- **Type-Safe**: Uses serde for strict JSON validation
- **Flexible**: Supports both explicit validation results and implicit structure validation
- **User-Defined**: POML files determine what structure to validate against

This approach provides validation similar to Pydantic in Python, but remains completely generic - the validator simply checks if valid JSON can be extracted from the response, without requiring specific schemas.

### Logging

All routing decisions are logged with `log_tx`:
- `"Agent X routing to node Y"` - Shows routing decisions
- `"Traversal X: Transitioning from node Y to node Z"` - Shows actual transitions
- `"Workflow completed (reached END node)"` - When `-1` route is taken

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

## Interactive Chat Mode

You can chat interactively with your selected workflow using the `/chat` command:

1. Select a workflow with `/workflow`
2. Enter chat mode with `/chat`
3. Type messages directly - they will be sent to the active workflow
4. Press ESC to exit chat mode

In chat mode, your messages are sent directly to the workflow without the "User:" prefix, enabling more natural conversation flow.

## Agent Selection

You can route your chat messages to specific agents within a workflow:

- `/agent list` - Show available agents in the current workflow
- `/agent <number>` - Select a specific agent by index (0-indexed)
- `/agent none` - Clear agent selection and use default workflow routing

Example:
```
/agent 0        # Route messages to the first agent
/agent 1        # Route messages to the second agent
/agent none     # Use default workflow routing
```

## Template Variables in POML Files

POML files now support template variables for dynamic content:

### Available Variables:
- `{{prompt}}` or `{{input}}` - The current input from the workflow

### Example POML with Variables:
```xml
<poml>
<user>
Please generate code for: {{prompt}}
The user specifically requested: {{input}}
</user>
</poml>
```

When the workflow runs, these variables will be replaced with actual values from your Rust program, enabling two-way communication between your application and the LLM.

---

## Roadmap

- Add `cat` tool for file reading
- Add streaming tool outputs
- Add UI for managing extensions

## Communicating with `<let>` Variables in POML

Neonmachines now fully supports **typed `<let>` variables** in `.poml` files.  
This allows you to **inject, overwrite, and update variables** dynamically from Rust.

---

### Default Variables

When a workflow runs, Neonmachines automatically injects two variables:

```xml
<let name="nminput" type="string" value="Original user input" />
<let name="nmoutput" type="string" value="Latest LLM output" />
```

- **`nminput`** → the original user input (first message in the workflow)  
- **`nmoutput`** → the latest LLM output (updated after each agent step)  

These are always present, even if not defined in your `.poml` file.

---

### Example POML with Variables

```xml
<poml>
  <let name="nminput" type="string" value="Generate Rust code" />
  <let name="nmoutput" type="string" value="User asked for a todo manager" />
  <let name="greeting" type="string" value="Hello, world!" />

  <user>
    Please generate code for: {{nminput}}
    The last output was: {{nmoutput}}
  </user>
</poml>
```

---

### Overwriting Variables

You can define your own `<let>` variables in `.poml` files, and Neonmachines will **overwrite them** if you pass values from Rust:

```xml
<let name="rustoverwrite" type="string" value="not overwritten yet" />
```

Rust can overwrite this with:

```xml
<let name="rustoverwrite" type="string" value="Rust has overwritten this" />
```

This ensures **type safety** and avoids POML parser errors.

---

### Supported `<let>` Forms

Neonmachines supports all POML `<let>` syntaxes:

1. **Simple value**
```xml
<let name="greeting" type="string" value="Hello, world!" />
<p>{{greeting}}</p>
```

2. **Inline content**
```xml
<let name="message" type="string">This is inline text</let>
<p>{{message}}</p>
```

3. **Import from file**
```xml
<let name="users" src="users.json" />
<p>First user: {{users[0].name}}</p>
```

4. **Anonymous import**
```xml
<let src="config.json" />
<p>API Key: {{apiKey}}</p>
```

5. **Inline JSON**
```xml
<let name="person" type="object">
  { "name": "Alice", "age": 30 }
</let>
<p>{{person.name}}</p>
```

6. **Expression**
```xml
<let name="base" type="number" value="10" />
<let name="increment" type="number" value="5" />
<let name="total" type="number" value="{{ base + increment }}" />
<p>Total: {{ total }}</p>
```

---

### Benefits

- **Two-way communication**:  
  POML defines variables, Rust can update them dynamically.  

- **Dynamic context injection**:  
  No more `{{prompt}}` placeholders — everything is managed via `<let>`.  

- **Type safety**:  
  All injected variables use `type="string"` by default, avoiding parser errors.  

- **Full POML compatibility**:  
  Works with all `<let>` syntaxes (value, inline, file, JSON, expression).  

---

### Logging

When a workflow starts, logs will show: