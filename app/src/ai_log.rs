//! AI Audit Log — tracks all scene modifications with AI/user attribution

use serde::{Serialize, Deserialize};
use std::collections::VecDeque;

const MAX_LOG_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiLogEntry {
    pub timestamp: String,
    pub actor: ActorId,
    pub action: String,
    pub details: String,
    pub objects_affected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorId {
    pub name: String,       // "Claude", "GPT-4o", "User", "Gemini"
    pub model: String,      // "claude-sonnet-4-20250514", "gpt-4o", "human"
    pub session_id: String, // unique per connection
}

impl ActorId {
    pub fn user() -> Self {
        Self { name: "\u{4f7f}\u{7528}\u{8005}".into(), model: "human".into(), session_id: "local".into() }
    }
    pub fn claude() -> Self {
        Self { name: "Claude".into(), model: "claude".into(), session_id: uuid::Uuid::new_v4().to_string()[..8].to_string() }
    }
    pub fn plugin(name: &str, model: &str) -> Self {
        Self { name: name.into(), model: model.into(), session_id: uuid::Uuid::new_v4().to_string()[..8].to_string() }
    }
    pub fn display_name(&self) -> String {
        if self.model == "human" {
            self.name.clone()
        } else {
            format!("{} ({})", self.name, self.model)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AiLog {
    entries: VecDeque<AiLogEntry>,
}

impl AiLog {
    pub fn new() -> Self { Self { entries: VecDeque::new() } }

    pub fn log(&mut self, actor: &ActorId, action: &str, details: &str, objects: Vec<String>) {
        let entry = AiLogEntry {
            timestamp: Self::now(),
            actor: actor.clone(),
            action: action.to_string(),
            details: details.to_string(),
            objects_affected: objects,
        };
        self.entries.push_back(entry);
        if self.entries.len() > MAX_LOG_ENTRIES {
            self.entries.pop_front();
        }
    }

    pub fn entries(&self) -> &VecDeque<AiLogEntry> { &self.entries }

    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let entries: Vec<&AiLogEntry> = self.entries.iter().collect();
        let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn now() -> String {
        // Simple timestamp
        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = elapsed.as_secs();
        let hours = (secs / 3600) % 24;
        let mins = (secs / 60) % 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, s)
    }

    pub fn clear(&mut self) { self.entries.clear(); }
}
