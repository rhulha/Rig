use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub providers: BTreeMap<String, Provider>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub models: Vec<Model>,
}

#[derive(Debug, Deserialize)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub compat: Compat,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Compat {
    #[serde(default)]
    pub max_tokens_field: Option<String>,
    #[serde(default)]
    pub supports_reasoning_effort: bool,
    /// Sampling temperature for this model. Omitted means greedy decoding.
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Nucleus and top-k sampling. Omitted leaves the provider's own default.
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub top_k: Option<u32>,
}

impl Compat {
    pub fn max_tokens_field(&self) -> &str {
        self.max_tokens_field.as_deref().unwrap_or("max_tokens")
    }
}

impl Provider {
    /// An apiKey of the form `${VAR}` is read from the environment, which the
    /// .env loader has already populated, so keys stay out of models.json.
    pub fn api_key(&self) -> Result<Option<String>, String> {
        let Some(value) = self.api_key.as_deref() else {
            return Ok(None);
        };
        let Some(var) = value.strip_prefix("${").and_then(|v| v.strip_suffix('}')) else {
            return Ok(Some(value.to_string()));
        };
        std::env::var(var)
            .map(Some)
            .map_err(|_| format!("{var} is referenced in models.json but is not set"))
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))
    }

    /// Pick a provider/model pair. Either name may be omitted; a bare model id
    /// is searched across all providers.
    pub fn resolve(
        &self,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<(&str, &Provider, &Model), String> {
        let candidates: Vec<(&str, &Provider)> = match provider {
            Some(name) => {
                let (key, p) = self
                    .providers
                    .get_key_value(name)
                    .ok_or_else(|| format!("no provider named '{name}'"))?;
                vec![(key.as_str(), p)]
            }
            None => self.providers.iter().map(|(k, v)| (k.as_str(), v)).collect(),
        };

        for (name, p) in &candidates {
            let hit = match model {
                Some(id) => p.models.iter().find(|m| m.id == id),
                None => p.models.first(),
            };
            if let Some(m) = hit {
                return Ok((name, p, m));
            }
        }

        Err(match model {
            Some(id) => format!("no model '{id}' in the selected provider(s)"),
            None => "no provider in models.json declares any models".to_string(),
        })
    }

    pub fn describe(&self) -> String {
        let mut out = String::new();
        for (name, p) in &self.providers {
            out.push_str(&format!("{name}  ({})\n", p.base_url));
            if p.models.is_empty() {
                out.push_str("    (no models declared)\n");
            }
            for m in &p.models {
                let tag = if m.reasoning { "  [reasoning]" } else { "" };
                out.push_str(&format!("    {}{}\n", m.id, tag));
            }
        }
        out
    }
}
