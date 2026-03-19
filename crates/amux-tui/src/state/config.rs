// Local wire type copies (will be replaced by crate::wire imports in Task 9)

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
}

impl ConfigState {
    pub fn new() -> Self {
        Self {
            provider: String::new(),
            base_url: String::new(),
            model: String::new(),
            api_key: String::new(),
            reasoning_effort: String::new(),
            fetched_models: Vec::new(),
            agent_config_raw: None,
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
