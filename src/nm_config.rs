use std::fs::{File};
use std::io::{Read, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentType {
    Agent,
    ParallelAgent,
}

#[derive(Debug, Clone)]
pub struct AgentRow {
    pub agent_type: AgentType,
    pub files: String,          // stores role:file mappings
    pub max_iterations: usize,
}

#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    pub name: String,
    pub rows: Vec<AgentRow>,
    pub active_agent_index: usize,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            name: "default".into(),
            rows: vec![AgentRow {
                agent_type: AgentType::Agent,
                files: String::new(),
                max_iterations: 3,
            }],
            active_agent_index: 0,
        }
    }
}

pub const CONFIG_FILE: &str = "config.nm";

pub fn save_nm(cfg: &WorkflowConfig) -> std::io::Result<()> {
    let mut out = String::new();
    out.push_str(&format!("workflow:{}\n", cfg.name));
    for (i, row) in cfg.rows.iter().enumerate() {
        out.push_str(&format!("agent_{}: {:?}\n", i + 1, row.agent_type));
        out.push_str(&format!("files:\"{}\"\n", row.files));
        out.push_str(&format!("maximum_iterations:{}\n", row.max_iterations));
    }
    let mut f = File::create(CONFIG_FILE)?;
    f.write_all(out.as_bytes())?;
    Ok(())
}

pub fn load_nm_or_create() -> WorkflowConfig {
    match load_nm() {
        Ok(cfg) => cfg,
        Err(_) => {
            let def = WorkflowConfig::default();
            let _ = save_nm(&def);
            def
        }
    }
}

fn load_nm() -> std::io::Result<WorkflowConfig> {
    let mut s = String::new();
    File::open(CONFIG_FILE)?.read_to_string(&mut s)?;
    parse_nm(&s)
}

fn parse_nm(s: &str) -> std::io::Result<WorkflowConfig> {
    let mut name = "default".to_string();
    let mut rows: Vec<AgentRow> = Vec::new();
    let mut cur_agent: Option<AgentRow> = None;

    let mut push_current =
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
        if let Some(rest) = line.strip_prefix("agent_") {
            push_current(&mut rows, &mut cur_agent);
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() == 2 {
                let ty = parts[1].trim();
                let agent_type = if ty.contains("Parallel") {
                    AgentType::ParallelAgent
                } else {
                    AgentType::Agent
                };
                cur_agent = Some(AgentRow {
                    agent_type,
                    files: String::new(),
                    max_iterations: 3,
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
    }
    push_current(&mut rows, &mut cur_agent);

    if rows.is_empty() {
        rows.push(AgentRow {
            agent_type: AgentType::Agent,
            files: String::new(),
            max_iterations: 3,
        });
    }

    Ok(WorkflowConfig {
        name,
        rows,
        active_agent_index: 0,
    })
}

pub fn preset_workflows() -> Vec<WorkflowConfig> {
    vec![
        WorkflowConfig::default(),
        WorkflowConfig {
            name: "security_checker".into(),
            rows: vec![
                AgentRow {
                    agent_type: AgentType::Agent,
                    files: "role:system:security_check.poml;role:user:get_files.poml".into(),
                    max_iterations: 3,
                },
                AgentRow {
                    agent_type: AgentType::ParallelAgent,
                    files: "role:ai:summary.poml".into(),
                    max_iterations: 2,
                },
            ],
            active_agent_index: 0,
        },
    ]
}