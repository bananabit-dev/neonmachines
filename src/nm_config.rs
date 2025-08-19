use std::fs::File;
use std::io::{Read, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentType {
    Agent,
    Validator, // Add this missing variant
    ParallelAgent,
}

#[derive(Debug, Clone)]
pub struct AgentRow {
    pub agent_type: AgentType,
    pub files: String,          // stores role:file mappings
    pub max_iterations: usize,
    pub on_success: Option<i32>,
    pub on_failure: Option<i32>,
    pub iteration_delay_ms: u64,   // ✅ configurable delay
    pub input_injections: Vec<String>,  // ✅ input injection patterns (e.g., "input1:output2")
    pub output_injections: Vec<String>, // ✅ output injection patterns
}

impl Default for AgentRow {
    fn default() -> Self {
        Self {
            agent_type: AgentType::Agent,
            files: String::new(),
            max_iterations: 3,
            on_success: None,
            on_failure: None,
            iteration_delay_ms: 200,
            input_injections: Vec::new(),
            output_injections: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    pub name: String,
    pub rows: Vec<AgentRow>,
    #[allow(dead_code)]  // Currently unused but may be needed for future features
    pub active_agent_index: usize,
    pub model: String,
    pub temperature: f32,
    pub maximum_traversals: usize,
    pub working_dir: String,   // ✅ new
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            name: "default".into(),
            rows: vec![AgentRow::default()],
            active_agent_index: 0,
            model: "z-ai/glm-4.5".into(),
            temperature: 0.7,
            maximum_traversals: 20,
            working_dir: ".".into(),   // ✅ default
        }
    }
}

pub const CONFIG_FILE: &str = "config.nm";

/// Save a single workflow (legacy compatibility)
pub fn save_nm(cfg: &WorkflowConfig) -> std::io::Result<()> {
    save_all_nm(std::slice::from_ref(cfg))
}

/// Save all workflows in multi-format
pub fn save_all_nm(cfgs: &[WorkflowConfig]) -> std::io::Result<()> {
    let mut out = String::new();
    for (i, cfg) in cfgs.iter().enumerate() {
        if i > 0 {
            out.push_str("\n====\n\n");
        }
        out.push_str(&format!("workflow:{}\n", cfg.name));
        out.push_str(&format!("model:{}\n", cfg.model));
        out.push_str(&format!("temperature:{}\n", cfg.temperature));
        out.push_str(&format!("maximum_traversals:{}\n", cfg.maximum_traversals));
        out.push_str(&format!("working_dir:{}\n", cfg.working_dir)); // ✅ save working_dir
        for (j, row) in cfg.rows.iter().enumerate() {
            out.push_str(&format!("agent_{}: {:?}\n", j + 1, row.agent_type));
            out.push_str(&format!("files:\"{}\"\n", row.files));
            out.push_str(&format!("maximum_iterations:{}\n", row.max_iterations));
            out.push_str(&format!("iteration_delay_ms:{}\n", row.iteration_delay_ms));
            out.push_str(&format!("on_success:{}\n", row.on_success.unwrap_or(-1)));
            out.push_str(&format!("on_failure:{}\n", row.on_failure.unwrap_or(-1)));
            // Save injection patterns
            if !row.input_injections.is_empty() {
                out.push_str(&format!("input_injections:\"{}\"\n", row.input_injections.join(";")));
            }
            if !row.output_injections.is_empty() {
                out.push_str(&format!("output_injections:\"{}\"\n", row.output_injections.join(";")));
            }
        }
    }
    let mut f = File::create(CONFIG_FILE)?;
    f.write_all(out.as_bytes())?;
    Ok(())
}

/// Load a single workflow (legacy compatibility)
#[allow(dead_code)]
pub fn load_nm_or_create() -> WorkflowConfig {
    #[cfg(debug_assertions)]
    eprintln!("Warning: load_nm_or_create() is deprecated, use load_all_nm() instead");
    match load_nm() {
        Ok(cfg) => cfg,
        Err(_) => {
            let def = WorkflowConfig::default();
            let _ = save_nm(&def);
            def
        }
    }
}

/// Load all workflows
pub fn load_all_nm() -> std::io::Result<Vec<WorkflowConfig>> {
    match load_all_nm_inner() {
        Ok(cfgs) => Ok(cfgs),
        Err(_) => {
            let def = WorkflowConfig::default();
            let _ = save_nm(&def);
            Ok(vec![def])
        }
    }
}

fn load_all_nm_inner() -> std::io::Result<Vec<WorkflowConfig>> {
    let mut s = String::new();
    File::open(CONFIG_FILE)?.read_to_string(&mut s)?;
    parse_nm_multiple(&s)
}

/// Load a single workflow (legacy compatibility)
#[allow(dead_code)]
fn load_nm() -> std::io::Result<WorkflowConfig> {
    let mut s = String::new();
    File::open(CONFIG_FILE)?.read_to_string(&mut s)?;
    let workflows = parse_nm_multiple(&s)?;
    workflows.into_iter().next().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "No workflows found")
    })
}

/// Parse multiple workflows separated by ====
pub fn parse_nm_multiple(s: &str) -> std::io::Result<Vec<WorkflowConfig>> {
    let mut workflows = Vec::new();
    let sections: Vec<&str> = s.split("====").collect();

    for section in sections {
        if section.trim().is_empty() {
            continue;
        }
        let workflow = parse_nm_single(section)?;
        workflows.push(workflow);
    }

    if workflows.is_empty() {
        workflows.push(parse_nm_single(s)?);
    }

    Ok(workflows)
}

fn parse_nm_single(s: &str) -> std::io::Result<WorkflowConfig> {
    let mut name = "default".to_string();
    let mut rows: Vec<AgentRow> = Vec::new();
    let mut cur_agent: Option<AgentRow> = None;
    let mut model = "z-ai/glm-4.5".to_string();
    let mut temperature = 0.7;
    let mut maximum_traversals = 20;
    let mut working_dir = ".".to_string(); // ✅ default

    let push_current =
        |rows: &mut Vec<AgentRow>, cur: &mut Option<AgentRow>| {
            if let Some(a) = cur.take() {
                rows.push(a);
            }
        };

    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("workflow:") {
            name = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("model:") {
            model = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("temperature:") {
            temperature = rest.trim().parse::<f32>().unwrap_or(0.7);
            continue;
        }
        if let Some(rest) = line.strip_prefix("maximum_traversals:") {
            maximum_traversals = rest.trim().parse::<usize>().unwrap_or(20);
            continue;
        }
        if let Some(rest) = line.strip_prefix("working_dir:") {
            working_dir = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("agent_") {
            push_current(&mut rows, &mut cur_agent);
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() == 2 {
                let ty = parts[1].trim();
                let agent_type = if ty.contains("Parallel") {
                    AgentType::ParallelAgent
                } else if ty.contains("Validator") {
                    AgentType::Validator
                } else {
                    AgentType::Agent
                };
                cur_agent = Some(AgentRow {
                    agent_type,
                    files: String::new(),
                    max_iterations: 3,
                    on_success: None,
                    on_failure: None,
                    iteration_delay_ms: 200,
                    input_injections: Vec::new(),
                    output_injections: Vec::new(),
                });
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("files:") {
            let val = rest.trim().trim_matches('"').to_string();
            if let Some(a) = &mut cur_agent {
                a.files = val;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("maximum_iterations:") {
            let n = rest.trim().parse::<usize>().unwrap_or(3);
            if let Some(a) = &mut cur_agent {
                a.max_iterations = n;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("iteration_delay_ms:") {
            let n = rest.trim().parse::<u64>().unwrap_or(200);
            if let Some(a) = &mut cur_agent {
                a.iteration_delay_ms = n;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("on_success:") {
            let n = rest.trim().parse::<i32>().unwrap_or(-1);
            if let Some(a) = &mut cur_agent {
                a.on_success = if n >= 0 { Some(n) } else { None };
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("on_failure:") {
            let n = rest.trim().parse::<i32>().unwrap_or(-1);
            if let Some(a) = &mut cur_agent {
                a.on_failure = if n >= 0 { Some(n) } else { None };
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("input_injections:") {
            let val = rest.trim().trim_matches('"').to_string();
            if let Some(a) = &mut cur_agent {
                a.input_injections = val.split(';').map(|s| s.trim().to_string()).collect();
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("output_injections:") {
            let val = rest.trim().trim_matches('"').to_string();
            if let Some(a) = &mut cur_agent {
                a.output_injections = val.split(';').map(|s| s.trim().to_string()).collect();
            }
            continue;
        }
    }
    push_current(&mut rows, &mut cur_agent);

    if rows.is_empty() {
        rows.push(AgentRow::default());
    }

    Ok(WorkflowConfig {
        name,
        rows,
        active_agent_index: 0,
        model,
        temperature,
        maximum_traversals,
        working_dir,
    })
}

pub fn preset_workflows() -> Vec<WorkflowConfig> {
    vec![WorkflowConfig::default()]
}

/// Process input/output injections for a given agent
pub fn process_injections(
    input: &str,
    agent: &AgentRow,
    shared_history: &crate::shared_history::SharedHistory,
    log_tx: &tokio::sync::mpsc::UnboundedSender<crate::runner::AppEvent>,
) -> String {
    let mut processed_input = input.to_string();
    
    // Process input injections (e.g., "input1:output2")
    for injection in &agent.input_injections {
        if let Some((source_input, target_output)) = injection.split_once(':') {
            let source_input = source_input.trim();
            let target_output = target_output.trim();
            
            // Find the source input from shared history
            if let Some(source_content) = find_input_from_history(source_input, shared_history) {
                // Inject the source content into the target output position
                processed_input = inject_into_input(&processed_input, target_output, &source_content);
                
                let _ = log_tx.send(crate::runner::AppEvent::Log(format!(
                    "[Injection] Injected '{}' from '{}' into input",
                    source_content, source_input
                )));
            }
        }
    }
    
    processed_input
}

/// Find content from shared history by input name
fn find_input_from_history(input_name: &str, shared_history: &crate::shared_history::SharedHistory) -> Option<String> {
    // Use the public API to get the full history
    let history_messages = shared_history.get_last(100); // Get last 100 messages
    
    // Look for messages that contain the input name
    for msg in &history_messages {
        if let Some(content) = &msg.content {
            if content.contains(&format!("input: {}", input_name)) || 
               content.contains(&format!("user: {}", input_name)) {
                // Extract the actual content after the pattern
                if let Some(pos) = content.find(input_name) {
                    let remaining = &content[pos + input_name.len()..];
                    if remaining.starts_with(": ") {
                        return Some(remaining[2..].to_string());
                    } else if remaining.starts_with(' ') {
                        return Some(remaining[1..].to_string());
                    }
                }
            }
        }
    }
    
    None
}

/// Inject content into input at specified position
fn inject_into_input(input: &str, position: &str, content: &str) -> String {
    if position == "start" {
        format!("{} {}", content, input)
    } else if position == "end" {
        format!("{} {}", input, content)
    } else if let Some(pos) = input.find(position) {
        let before = &input[..pos];
        let after = &input[pos..];
        format!("{} {} {}", before, content, after)
    } else {
        input.to_string()
    }
}