# Neonmachines Ignore and Hints Files

## .nmignore

The `.nmignore` file specifies files and directories that should be ignored by Neonmachines when processing projects. It uses the same syntax as `.gitignore`.

### Purpose
- Exclude files that shouldn't be processed by AI tools
- Reduce noise in AI analysis
- Protect sensitive files
- Improve performance by skipping unnecessary files

### Syntax
```
# Comment lines start with #
*.log           # Ignore all .log files
logs/           # Ignore entire logs directory
*.tmp           # Ignore temporary files
.env            # Ignore environment files
```

### Common Patterns
- `*.extension` - Ignore all files with a specific extension
- `directory/` - Ignore entire directory
- `filename` - Ignore specific file
- `!filename` - Don't ignore specific file (whitelist)

## .nmhints

The `.nmhints` file provides hints to Neonmachines about how to process different types of files. It helps the system make better decisions about AI processing approaches.

### Format
```
file_pattern:hint_type:hint_value
```

### Available Hint Types

#### Language Hints
```
*.py:language:python
*.rs:language:rust
```
Tells Neonmachines what programming language a file contains.

#### Documentation Hints
```
README.md:doc:true
*.md:doc:true
```
Marks files that contain documentation content.

#### Test Hints
```
*_test.py:test:true
tests/*:test:true
```
Identifies test files and directories.

#### Configuration Hints
```
*.nm:config:true
*.nmmcpextension:config:true
```
Marks configuration files used by Neonmachines.

#### AI Processing Hints
```
*.md:ai_approach:documentation
*.py:ai_approach:code_review
prompts/*.poml:ai_approach:prompt_engineering
```
Suggests the best AI processing approach for different file types.

#### Priority Hints
```
README.md:priority:high
*.py:priority:medium
*.md:priority:low
```
Sets processing priority for different files.

### Usage Example

A typical project structure with hints:
```
project/
├── .nmignore          # Ignore logs, temp files, etc.
├── .nmhints           # Processing hints for AI
├── README.md          # High priority documentation
├── src/
│   ├── main.py        # Python code files
│   └── tests/         # Test files
├── prompts/
│   └── workflow.poml  # Prompt engineering approach
└── config.nm          # High priority configuration
```

With these hints, Neonmachines will:
1. Skip ignored files during processing
2. Apply appropriate AI processing approaches to different file types
3. Prioritize important files like README.md and config.nm
4. Recognize and handle test files differently from main code
