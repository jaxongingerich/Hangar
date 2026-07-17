use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SIDECAR_DIR: &str = ".hangar";
pub const SIDECAR_FILE: &str = "project.json";
pub const LOG_FILE: &str = "log.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sidecar {
    #[serde(default = "default_version")]
    pub version: u32,
    pub name: String,
    #[serde(default = "default_emoji")]
    pub emoji: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub progress: i64,
    #[serde(default = "default_progress_mode")]
    pub progress_mode: String,
    #[serde(default)]
    pub target_date: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

fn default_version() -> u32 {
    1
}
fn default_emoji() -> String {
    "📦".into()
}
fn default_color() -> String {
    "#22D3A6".into()
}
fn default_status() -> String {
    "active".into()
}
fn default_progress_mode() -> String {
    "manual".into()
}

const CARD_COLORS: &[&str] = &[
    "#22D3A6", "#8B5CF6", "#38BDF8", "#F5A524", "#F472B6", "#A3E635", "#FB923C",
];

impl Sidecar {
    pub fn new(name: &str) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        // Deterministic color per name so rescans stay stable
        let hash: usize = name.bytes().map(|b| b as usize).sum();
        Sidecar {
            version: 1,
            name: name.to_string(),
            emoji: default_emoji(),
            color: CARD_COLORS[hash % CARD_COLORS.len()].to_string(),
            status: default_status(),
            progress: 0,
            progress_mode: default_progress_mode(),
            target_date: None,
            pinned: false,
            tags: vec![],
            created_at: Some(now.clone()),
            updated_at: Some(now),
        }
    }

    pub fn load(project_dir: &Path) -> Option<Sidecar> {
        let path = project_dir.join(SIDECAR_DIR).join(SIDECAR_FILE);
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self, project_dir: &Path) -> AppResult<()> {
        let dir = project_dir.join(SIDECAR_DIR);
        std::fs::create_dir_all(&dir)?;
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(dir.join(SIDECAR_FILE), text)?;
        Ok(())
    }

    /// Load the sidecar if present, otherwise create + persist a fresh one.
    pub fn load_or_init(project_dir: &Path, name: &str) -> AppResult<Sidecar> {
        if let Some(sc) = Sidecar::load(project_dir) {
            return Ok(sc);
        }
        let sc = Sidecar::new(name);
        sc.save(project_dir)?;
        Ok(sc)
    }
}

pub fn append_log_md(project_dir: &Path, line: &str) -> AppResult<()> {
    use std::io::Write;
    let dir = project_dir.join(SIDECAR_DIR);
    std::fs::create_dir_all(&dir)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(LOG_FILE))?;
    writeln!(f, "{}", line)?;
    Ok(())
}

pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out = "project".into();
    }
    out
}
