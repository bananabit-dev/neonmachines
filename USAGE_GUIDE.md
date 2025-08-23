# Neonmachines Advanced Usage Guide

## Preprompting with Secondary Agents

Neonmachines now supports preprompting, allowing you to trigger secondary agents with specific inputs. This enables complex multi-step workflows.

### Syntax
```
primary task input2="secondary task here"
```

### Examples

1. **Code Generation with Optimization**
   ```
   create a fibonacci function input2="optimize for performance with memoization"
   ```

2. **Web Scraper with Error Handling**
   ```
   build a web scraper input2="add rate limiting and error handling"
   ```

3. **Code Analysis with Security Check**
   ```
   analyze this code input2="check for security vulnerabilities"
   ```

## Available Workflows

### 1. Coding Agent Workflow (`coding_agent.poml`)
- Primary agent for general coding tasks
- Supports preprompting with secondary agents

### 2. Security Analyzer Workflow (`security_analyzer.poml`)
- Specialized for security vulnerability detection
- Identifies OWASP Top 10 vulnerabilities
- Provides remediation strategies

### 3. Advanced Coding Workflow (`advanced_coding_workflow.nm`)
Multi-agent workflow with:
- Primary coding agent
- Security analyzer agent
- Code validator agent
- Documentation generator agent

## Special Commands

### `/help preprompting`
Shows detailed help for preprompting syntax and examples

### `/run security_analyzer`
Run the security vulnerability analyzer on your code

### `/run advanced_coding_workflow`
Execute the complete multi-agent coding workflow

## Security Features

### Vulnerability Detection
The security analyzer can detect:
- SQL Injection
- Cross-Site Scripting (XSS)
- Cross-Site Request Forgery (CSRF)
- Security Misconfiguration
- Sensitive Data Exposure
- And 5 more OWASP Top 10 categories

### Remediation Guidance
For each vulnerability, you get:
- Detailed explanation
- Exploitation examples
- Secure coding solutions
- Best practice recommendations

## Web UI Features

### Theme Selection
Switch between:
- Default neon theme
- Professional white paper theme

### Real-time Metrics
Monitor:
- Request counts
- Success rates
- Response times
- Active requests
- System alerts

### Request Tracing
View detailed traces of:
- API calls
- Service interactions
- Request durations
- Success/failure status

## Workflow Orchestration

Multiple agents can work together:
1. Primary agent generates initial solution
2. Secondary agent optimizes/refactors
3. Security agent validates for vulnerabilities
4. Validator agent checks syntax and logic
5. Documentation agent creates documentation

This enables complex AI-powered development workflows with human-in-the-loop validation.
