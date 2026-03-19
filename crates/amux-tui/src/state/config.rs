// Local wire type copies (will be replaced by crate::wire imports in Task 9)
#![allow(dead_code)]

#[derive(Debug, Clone, Default)]
pub struct FetchedModel {
    pub id: String,
    pub name: Option<String>,
    pub context_window: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct AgentConfigSnapshot {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub reasoning_effort: String,
}

// ── ConfigAction ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ConfigAction {
    ConfigReceived(AgentConfigSnapshot),
    ConfigRawReceived(serde_json::Value),
    ModelsFetched(Vec<FetchedModel>),
    SetProvider(String),
    SetModel(String),
    SetReasoningEffort(String),
    ToggleTool(String), // toggle tool by name: "bash", "file_ops", "web_search", etc.
}

// ── ConfigState ───────────────────────────────────────────────────────────────

pub struct ConfigState {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub reasoning_effort: String,
    pub fetched_models: Vec<FetchedModel>,
    pub agent_config_raw: Option<serde_json::Value>,

    // Tool toggles
    pub tool_bash: bool,
    pub tool_file_ops: bool,
    pub tool_web_search: bool,
    pub tool_web_browse: bool,
    pub tool_vision: bool,
    pub tool_system_info: bool,
    pub tool_gateway: bool,

    // Web search config
    pub search_provider: String,    // "none", "firecrawl", "exa", "tavily"
    pub firecrawl_api_key: String,
    pub exa_api_key: String,
    pub tavily_api_key: String,
    pub search_max_results: u32,
    pub search_timeout_secs: u32,

    // Gateway config
    pub gateway_enabled: bool,
    pub slack_token: String,
    pub telegram_token: String,
    pub discord_token: String,
    pub gateway_prefix: String,
}

impl ConfigState {
    pub fn new() -> Self {
        Self {
            provider: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-5.4".to_string(),
            api_key: String::new(),
            reasoning_effort: String::new(),
            fetched_models: Vec::new(),
            agent_config_raw: None,
            tool_bash: true,
            tool_file_ops: true,
            tool_web_search: true,
            tool_web_browse: false,
            tool_vision: false,
            tool_system_info: true,
            tool_gateway: false,
            search_provider: "none".to_string(),
            firecrawl_api_key: String::new(),
            exa_api_key: String::new(),
            tavily_api_key: String::new(),
            search_max_results: 8,
            search_timeout_secs: 20,
            gateway_enabled: false,
            slack_token: String::new(),
            telegram_token: String::new(),
            discord_token: String::new(),
            gateway_prefix: "!tamux".to_string(),
        }
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn reasoning_effort(&self) -> &str {
        &self.reasoning_effort
    }

    pub fn fetched_models(&self) -> &[FetchedModel] {
        &self.fetched_models
    }

    pub fn agent_config_raw(&self) -> Option<&serde_json::Value> {
        self.agent_config_raw.as_ref()
    }

    pub fn reduce(&mut self, action: ConfigAction) {
        match action {
            ConfigAction::ConfigReceived(snapshot) => {
                self.provider = snapshot.provider;
                self.base_url = snapshot.base_url;
                self.model = snapshot.model;
                self.api_key = snapshot.api_key;
                self.reasoning_effort = snapshot.reasoning_effort;
            }

            ConfigAction::ConfigRawReceived(raw) => {
                self.agent_config_raw = Some(raw);
            }

            ConfigAction::ModelsFetched(models) => {
                self.fetched_models = models;
            }

            ConfigAction::SetProvider(provider) => {
                self.provider = provider;
            }

            ConfigAction::SetModel(model) => {
                self.model = model;
            }

            ConfigAction::SetReasoningEffort(effort) => {
                self.reasoning_effort = effort;
            }
            ConfigAction::ToggleTool(name) => {
                match name.as_str() {
                    "bash" => self.tool_bash = !self.tool_bash,
                    "file_ops" => self.tool_file_ops = !self.tool_file_ops,
                    "web_search" => self.tool_web_search = !self.tool_web_search,
                    "web_browse" => self.tool_web_browse = !self.tool_web_browse,
                    "vision" => self.tool_vision = !self.tool_vision,
                    "system_info" => self.tool_system_info = !self.tool_system_info,
                    "gateway" => self.tool_gateway = !self.tool_gateway,
                    _ => {}
                }
            }
        }
    }
}

impl Default for ConfigState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(provider: &str, model: &str) -> AgentConfigSnapshot {
        AgentConfigSnapshot {
            provider: provider.into(),
            model: model.into(),
            base_url: "https://api.example.com".into(),
            api_key: "sk-test".into(),
            reasoning_effort: "high".into(),
        }
    }

    #[test]
    fn config_received_populates_all_fields() {
        let mut state = ConfigState::new();
        state.reduce(ConfigAction::ConfigReceived(make_snapshot("openai", "gpt-4o")));
        assert_eq!(state.provider(), "openai");
        assert_eq!(state.model(), "gpt-4o");
        assert_eq!(state.base_url(), "https://api.example.com");
        assert_eq!(state.api_key(), "sk-test");
        assert_eq!(state.reasoning_effort(), "high");
    }

    #[test]
    fn models_fetched_replaces_list() {
        let mut state = ConfigState::new();
        state.reduce(ConfigAction::ModelsFetched(vec![
            FetchedModel { id: "m1".into(), name: Some("Model One".into()), context_window: Some(128_000) },
            FetchedModel { id: "m2".into(), name: None, context_window: None },
        ]));
        assert_eq!(state.fetched_models().len(), 2);
        assert_eq!(state.fetched_models()[0].id, "m1");

        state.reduce(ConfigAction::ModelsFetched(vec![]));
        assert_eq!(state.fetched_models().len(), 0);
    }

    #[test]
    fn set_provider_updates_only_provider() {
        let mut state = ConfigState::new();
        state.reduce(ConfigAction::ConfigReceived(make_snapshot("openai", "gpt-4o")));
        state.reduce(ConfigAction::SetProvider("anthropic".into()));
        assert_eq!(state.provider(), "anthropic");
        // Other fields unchanged
        assert_eq!(state.model(), "gpt-4o");
        assert_eq!(state.api_key(), "sk-test");
    }

    #[test]
    fn set_model_updates_only_model() {
        let mut state = ConfigState::new();
        state.reduce(ConfigAction::ConfigReceived(make_snapshot("openai", "gpt-4o")));
        state.reduce(ConfigAction::SetModel("gpt-4o-mini".into()));
        assert_eq!(state.model(), "gpt-4o-mini");
        assert_eq!(state.provider(), "openai");
    }

    #[test]
    fn config_raw_received_stores_json() {
        let mut state = ConfigState::new();
        assert!(state.agent_config_raw().is_none());

        let raw = serde_json::json!({ "key": "value", "number": 42 });
        state.reduce(ConfigAction::ConfigRawReceived(raw.clone()));
        assert!(state.agent_config_raw().is_some());
        assert_eq!(state.agent_config_raw().unwrap()["key"], "value");
    }

    #[test]
    fn config_received_twice_overwrites() {
        let mut state = ConfigState::new();
        state.reduce(ConfigAction::ConfigReceived(make_snapshot("openai", "gpt-4o")));
        state.reduce(ConfigAction::ConfigReceived(make_snapshot("anthropic", "claude-3-5-sonnet")));
        assert_eq!(state.provider(), "anthropic");
        assert_eq!(state.model(), "claude-3-5-sonnet");
    }
}
