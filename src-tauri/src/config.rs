//! `config.json`: provider settings and translation behavior. Structured so
//! the frontend only ever sees a redacted view (`PublicConfig` — a boolean
//! "is a key configured", never the key itself) and can only *overwrite*
//! a provider's key, never read the existing one back.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    AnthropicMessages,
    OpenAiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    /// A value *expression*, not necessarily the literal secret: `$ENV_VAR`,
    /// `!command`, or a literal string. See [`resolve_key`].
    pub api_key: String,
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_output_tokens: u32,
}

fn default_max_tokens() -> u32 {
    8192
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSettings {
    #[serde(default)]
    pub auto_translate_on_change: bool,
    #[serde(default = "default_true")]
    pub check_on_startup: bool,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_chunk_threshold")]
    pub chunk_threshold_chars: usize,
}

fn default_true() -> bool {
    true
}
fn default_concurrency() -> u32 {
    2
}
fn default_chunk_threshold() -> usize {
    12_000
}

impl Default for TranslationSettings {
    fn default() -> Self {
        Self {
            auto_translate_on_change: false,
            check_on_startup: true,
            concurrency: default_concurrency(),
            chunk_threshold_chars: default_chunk_threshold(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub active_provider: Option<String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub translation: TranslationSettings,
    #[serde(default)]
    pub github: GithubConfig,
}

/// GitHub access settings: the GitHub API is anonymous-rate-limited to 60
/// requests/hour, which importing a couple of remote sources exhausts. An
/// optional token (a resolve_key expression, so `$ENV_VAR`/`!command` work
/// like provider api keys) lifts it to 5000/h; when absent, the login from
/// the local `gh` CLI can be reused instead (opt-out, default on).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GithubConfig {
    /// A resolve_key expression ($ENV / !command / literal) or empty.
    pub token: String,
    /// Try `gh auth token` when no token expression is set.
    pub use_gh_cli: bool,
}

impl Default for GithubConfig {
    fn default() -> Self {
        GithubConfig { token: String::new(), use_gh_cli: true }
    }
}

/// Pure precedence core for GitHub auth: token expression > gh CLI output
/// (already trimmed by the caller) > anonymous. Returns None when nothing
/// yields a usable token.
pub fn resolve_github_token(
    token_expr: &str,
    use_gh_cli: bool,
    gh_cli_token: Option<&str>,
) -> Option<String> {
    if !token_expr.trim().is_empty() {
        return resolve_key(token_expr).ok();
    }
    if use_gh_cli {
        gh_cli_token.map(|t| t.to_string())
    } else {
        None
    }
}

/// The token to attach to GitHub requests for this config, running
/// `gh auth token` when allowed. A missing `gh` binary or a not-logged-in
/// state silently falls back to anonymous.
pub fn github_token_for(cfg: &AppConfig) -> Option<String> {
    let gh_cli = if cfg.github.use_gh_cli { gh_auth_token() } else { None };
    resolve_github_token(&cfg.github.token, cfg.github.use_gh_cli, gh_cli.as_deref())
}

fn gh_auth_token() -> Option<String> {
    let out = std::process::Command::new("gh").args(["auth", "token"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

pub fn config_path(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

/// Load `config.json`, or the all-defaults `AppConfig` if it doesn't exist yet.
pub fn load_config(dir: &Path) -> Result<AppConfig> {
    let path = config_path(dir);
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let content = std::fs::read_to_string(&path).with_context(|| format!("reading {path:?}"))?;
    serde_json::from_str(&content).with_context(|| format!("parsing {path:?}"))
}

/// Same write-temp-then-rename atomicity as `state::save_state_atomic`.
pub fn save_config_atomic(dir: &Path, config: &AppConfig) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let final_path = config_path(dir);
    let tmp_path = dir.join("config.json.tmp");
    let json = serde_json::to_string_pretty(config).context("serializing config")?;
    std::fs::write(&tmp_path, json).with_context(|| format!("writing {tmp_path:?}"))?;
    std::fs::rename(&tmp_path, &final_path).with_context(|| format!("renaming into {final_path:?}"))?;
    Ok(())
}

/// Resolve a config value expression into its actual value:
/// - `$ENV_VAR` reads an environment variable
/// - `!command` runs a shell command and takes its stdout (for reading a
///   system keychain, e.g. macOS `security find-generic-password`)
/// - anything else is used as a literal
pub fn resolve_key(expr: &str) -> Result<String> {
    let trimmed = expr.trim();
    if let Some(var) = trimmed.strip_prefix('$') {
        std::env::var(var).with_context(|| format!("environment variable '{var}' is not set"))
    } else if let Some(cmd) = trimmed.strip_prefix('!') {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .with_context(|| format!("running command: {cmd}"))?;
        if !output.status.success() {
            bail!("command '{cmd}' exited with status {}", output.status);
        }
        let text = String::from_utf8(output.stdout).context("command output is not valid UTF-8")?;
        Ok(text.trim().to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

// --- Redacted view for the frontend ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub has_api_key: bool,
    pub model: String,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicConfig {
    pub active_provider: Option<String>,
    pub providers: HashMap<String, PublicProviderConfig>,
    pub translation: TranslationSettings,
    #[serde(default)]
    pub github: PublicGithubConfig,
}

/// Redacted GitHub section for the frontend: whether a token expression is
/// stored (never the value itself) and the gh-CLI opt-in flag.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicGithubConfig {
    pub has_token: bool,
    pub use_gh_cli: bool,
}

impl From<&AppConfig> for PublicConfig {
    fn from(cfg: &AppConfig) -> Self {
        PublicConfig {
            active_provider: cfg.active_provider.clone(),
            providers: cfg
                .providers
                .iter()
                .map(|(id, p)| {
                    (
                        id.clone(),
                        PublicProviderConfig {
                            kind: p.kind,
                            base_url: p.base_url.clone(),
                            has_api_key: !p.api_key.trim().is_empty(),
                            model: p.model.clone(),
                            max_output_tokens: p.max_output_tokens,
                        },
                    )
                })
                .collect(),
            translation: cfg.translation.clone(),
            github: PublicGithubConfig {
                has_token: !cfg.github.token.trim().is_empty(),
                use_gh_cli: cfg.github.use_gh_cli,
            },
        }
    }
}

// --- What the frontend sends back on save ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    pub kind: ProviderKind,
    pub base_url: String,
    /// `None` (or blank) means "leave the existing key untouched" — the
    /// frontend never has the real key to send back in the first place.
    pub api_key: Option<String>,
    pub model: String,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInput {
    pub active_provider: Option<String>,
    pub providers: HashMap<String, ProviderInput>,
    pub translation: TranslationSettings,
    pub github: GithubInput,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubInput {
    /// `None`/blank means "keep the existing token expression" — same
    /// semantics as provider api keys: the frontend never sends it back.
    pub token: Option<String>,
    pub use_gh_cli: bool,
}
/// preserving each provider's stored key when the input didn't supply a new one.
pub fn apply_config_input(existing: &AppConfig, input: ConfigInput) -> AppConfig {
    let providers = input
        .providers
        .into_iter()
        .map(|(id, p)| {
            let api_key = match p.api_key {
                Some(expr) if !expr.trim().is_empty() => expr,
                _ => existing.providers.get(&id).map(|old| old.api_key.clone()).unwrap_or_default(),
            };
            (
                id,
                ProviderConfig {
                    kind: p.kind,
                    base_url: p.base_url,
                    api_key,
                    model: p.model,
                    max_output_tokens: p.max_output_tokens,
                },
            )
        })
        .collect();

    AppConfig {
        active_provider: input.active_provider,
        providers,
        translation: input.translation,
        github: GithubConfig {
            token: match input.github.token {
                Some(t) if !t.trim().is_empty() => t,
                _ => existing.github.token.clone(),
            },
            use_gh_cli: input.github.use_gh_cli,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_key_reads_env_var() {
        std::env::set_var("DOCSWIKI_TEST_KEY", "secret-value");
        assert_eq!(resolve_key("$DOCSWIKI_TEST_KEY").unwrap(), "secret-value");
        std::env::remove_var("DOCSWIKI_TEST_KEY");
    }

    #[test]
    fn resolve_key_errors_on_missing_env_var() {
        std::env::remove_var("DOCSWIKI_DEFINITELY_UNSET");
        assert!(resolve_key("$DOCSWIKI_DEFINITELY_UNSET").is_err());
    }

    #[test]
    fn resolve_key_runs_command_and_trims_output() {
        assert_eq!(resolve_key("!echo hello").unwrap(), "hello");
    }

    #[test]
    fn resolve_key_errors_on_failing_command() {
        assert!(resolve_key("!exit 1").is_err());
    }

    #[test]
    fn resolve_key_treats_plain_text_as_literal() {
        assert_eq!(resolve_key("sk-literal-value").unwrap(), "sk-literal-value");
    }

    fn sample_provider(api_key: &str) -> ProviderConfig {
        ProviderConfig {
            kind: ProviderKind::AnthropicMessages,
            base_url: "https://api.anthropic.com".to_string(),
            api_key: api_key.to_string(),
            model: "claude-sonnet-4-5".to_string(),
            max_output_tokens: 8192,
        }
    }

    #[test]
    fn public_config_redacts_the_api_key() {
        let mut cfg = AppConfig::default();
        cfg.providers.insert("anthropic".to_string(), sample_provider("$ANTHROPIC_API_KEY"));
        let public = PublicConfig::from(&cfg);
        let provider = &public.providers["anthropic"];
        assert!(provider.has_api_key);
        // PublicProviderConfig has no api_key field at all — this is
        // enforced by the type, not just by omitting it here.
        let json = serde_json::to_string(&public).unwrap();
        assert!(!json.contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn apply_config_input_preserves_key_when_none_supplied() {
        let mut existing = AppConfig::default();
        existing.providers.insert("anthropic".to_string(), sample_provider("$ANTHROPIC_API_KEY"));

        let input = ConfigInput {
            active_provider: Some("anthropic".to_string()),
            providers: HashMap::from([(
                "anthropic".to_string(),
                ProviderInput {
                    kind: ProviderKind::AnthropicMessages,
                    base_url: "https://api.anthropic.com".to_string(),
                    api_key: None, // left blank in the form
                    model: "claude-opus-4".to_string(), // model was changed though
                    max_output_tokens: 4096,
                },
            )]),
            translation: TranslationSettings::default(),
            github: GithubInput { token: None, use_gh_cli: true },
        };

        let merged = apply_config_input(&existing, input);
        let provider = &merged.providers["anthropic"];
        assert_eq!(provider.api_key, "$ANTHROPIC_API_KEY");
        assert_eq!(provider.model, "claude-opus-4");
    }

    #[test]
    fn apply_config_input_overwrites_key_when_supplied() {
        let mut existing = AppConfig::default();
        existing.providers.insert("anthropic".to_string(), sample_provider("$OLD_KEY"));

        let input = ConfigInput {
            active_provider: Some("anthropic".to_string()),
            providers: HashMap::from([(
                "anthropic".to_string(),
                ProviderInput {
                    kind: ProviderKind::AnthropicMessages,
                    base_url: "https://api.anthropic.com".to_string(),
                    api_key: Some("$NEW_KEY".to_string()),
                    model: "claude-sonnet-4-5".to_string(),
                    max_output_tokens: 8192,
                },
            )]),
            translation: TranslationSettings::default(),
            github: GithubInput { token: None, use_gh_cli: true },
        };

        let merged = apply_config_input(&existing, input);
        assert_eq!(merged.providers["anthropic"].api_key, "$NEW_KEY");
    }

    #[test]
    fn config_round_trips_through_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = AppConfig::default();
        cfg.active_provider = Some("anthropic".to_string());
        cfg.providers.insert("anthropic".to_string(), sample_provider("$ANTHROPIC_API_KEY"));

        save_config_atomic(tmp.path(), &cfg).unwrap();
        let loaded = load_config(tmp.path()).unwrap();
        assert_eq!(loaded.active_provider, Some("anthropic".to_string()));
        assert_eq!(loaded.providers["anthropic"].model, "claude-sonnet-4-5");
    }

    #[test]
    fn missing_config_file_yields_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = load_config(tmp.path()).unwrap();
        assert!(cfg.providers.is_empty());
        assert!(cfg.translation.check_on_startup);
    }

    #[test]
    fn github_token_expression_wins_over_gh_cli() {
        assert_eq!(
            resolve_github_token("literal-token", true, Some("gho_cli")),
            Some("literal-token".to_string())
        );
        // gh CLI used only when no expression is set
        assert_eq!(resolve_github_token("", true, Some("gho_cli")), Some("gho_cli".to_string()));
        // opt-out flag skips the CLI
        assert_eq!(resolve_github_token("", false, Some("gho_cli")), None);
        // nothing configured at all -> anonymous
        assert_eq!(resolve_github_token("", true, None), None);
    }

    #[test]
    fn blank_github_token_input_preserves_existing_one() {
        let mut existing = AppConfig::default();
        existing.github.token = "$MY_GH_TOKEN".to_string();
        existing.github.use_gh_cli = false;
        let input = ConfigInput {
            active_provider: None,
            providers: HashMap::new(),
            translation: TranslationSettings::default(),
            github: GithubInput { token: None, use_gh_cli: true },
        };
        let merged = apply_config_input(&existing, input);
        assert_eq!(merged.github.token, "$MY_GH_TOKEN");
        assert!(merged.github.use_gh_cli);
    }
}
