use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env, fs,
    path::PathBuf,
};

use serde::{Deserialize, Deserializer, de};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl Default for LineEnding {
    fn default() -> Self {
        Self::Lf
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IndentationConfig {
    pub tab_width: u8,
    pub indent_width: u8,
    pub use_tabs: bool,
}

impl Default for IndentationConfig {
    fn default() -> Self {
        Self {
            tab_width: 4,
            indent_width: 4,
            use_tabs: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeColors {
    pub foreground: String,
    pub background: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            foreground: "#d4d4d4".to_string(),
            background: "#1e1e1e".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EditorConfig {
    pub indentation: IndentationConfig,
    pub default_line_ending: LineEnding,
    pub theme: ThemeColors,
    #[serde(
        default = "EditorConfig::default_keybindings",
        deserialize_with = "deserialize_keybindings"
    )]
    pub keybindings: BTreeMap<String, String>,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            indentation: IndentationConfig::default(),
            default_line_ending: LineEnding::default(),
            theme: ThemeColors::default(),
            keybindings: Self::default_keybindings(),
        }
    }
}

impl EditorConfig {
    pub fn load(config_override: Option<PathBuf>) -> Result<Self, ConfigError> {
        let config_path = resolve_config_path(config_override)?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = match fs::read_to_string(&config_path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(source) => {
                return Err(ConfigError::Io {
                    path: config_path,
                    source,
                });
            }
        };

        let config = serde_yaml::from_str::<EditorConfig>(&content).map_err(|source| {
            if let Some(command) = parse_duplicate_keybinding_error(&source) {
                return ConfigError::DuplicateKeybinding { command };
            }
            ConfigError::Parse {
                path: config_path.clone(),
                source,
            }
        })?;

        config.validate()
    }

    fn default_keybindings() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("quit".to_string(), "Ctrl+Q".to_string()),
            ("save".to_string(), "Ctrl+S".to_string()),
        ])
    }

    fn validate(mut self) -> Result<Self, ConfigError> {
        validate_width(self.indentation.tab_width, "indentation.tab_width")?;
        validate_width(self.indentation.indent_width, "indentation.indent_width")?;
        validate_hex_color(&self.theme.foreground, "theme.foreground")?;
        validate_hex_color(&self.theme.background, "theme.background")?;

        let mut normalized = HashMap::new();
        for (command, keystroke) in &mut self.keybindings {
            if command.trim().is_empty() {
                return Err(ConfigError::InvalidKeybinding {
                    command: command.clone(),
                    keystroke: keystroke.clone(),
                    reason: "command name cannot be empty".to_string(),
                });
            }

            let normalized_keystroke = normalize_keystroke(keystroke).map_err(|reason| {
                ConfigError::InvalidKeybinding {
                    command: command.clone(),
                    keystroke: keystroke.clone(),
                    reason,
                }
            })?;

            if let Some(existing_command) =
                normalized.insert(normalized_keystroke.clone(), command.clone())
            {
                return Err(ConfigError::KeybindingConflict {
                    keystroke: normalized_keystroke,
                    first_command: existing_command,
                    second_command: command.clone(),
                });
            }

            *keystroke = normalized_keystroke;
        }

        Ok(self)
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to resolve config path: {0}")]
    PathResolution(String),
    #[error("failed to read config file `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML config `{path}`: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("duplicate keybinding for command `{command}`")]
    DuplicateKeybinding { command: String },
    #[error("invalid value for `{field}`: {message}")]
    Validation {
        field: &'static str,
        message: String,
    },
    #[error("invalid keybinding `{keystroke}` for command `{command}`: {reason}")]
    InvalidKeybinding {
        command: String,
        keystroke: String,
        reason: String,
    },
    #[error(
        "keybinding conflict: `{keystroke}` is assigned to both `{first_command}` and `{second_command}`"
    )]
    KeybindingConflict {
        keystroke: String,
        first_command: String,
        second_command: String,
    },
}

fn resolve_config_path(config_override: Option<PathBuf>) -> Result<PathBuf, ConfigError> {
    if let Some(path) = config_override {
        return Ok(path);
    }

    if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME")
        && !xdg_config_home.is_empty()
    {
        return Ok(PathBuf::from(xdg_config_home)
            .join("ebba")
            .join("config.yaml"));
    }

    let home_dir = dirs::home_dir().ok_or_else(|| {
        ConfigError::PathResolution(
            "unable to determine home directory for fallback config path".to_string(),
        )
    })?;

    Ok(home_dir.join(".config").join("ebba").join("config.yaml"))
}

fn validate_width(value: u8, field: &'static str) -> Result<(), ConfigError> {
    if value == 0 {
        return Err(ConfigError::Validation {
            field,
            message: "must be greater than 0".to_string(),
        });
    }

    Ok(())
}

fn validate_hex_color(value: &str, field: &'static str) -> Result<(), ConfigError> {
    let valid = value.len() == 7
        && value.starts_with('#')
        && value.chars().skip(1).all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err(ConfigError::Validation {
            field,
            message: format!("`{value}` is not a valid #RRGGBB color"),
        });
    }
    Ok(())
}

fn normalize_keystroke(raw: &str) -> Result<String, String> {
    let tokens: Vec<&str> = raw.split('+').map(str::trim).collect();
    if tokens.is_empty() || tokens.iter().any(|token| token.is_empty()) {
        return Err("use a format like `Ctrl+S`".to_string());
    }

    let mut modifiers_seen = HashSet::new();
    let mut modifiers = Vec::new();
    for token in &tokens[..tokens.len().saturating_sub(1)] {
        let canonical = canonical_modifier(token)
            .ok_or_else(|| format!("unknown modifier `{token}` (expected Ctrl/Alt/Shift/Meta)"))?;
        if !modifiers_seen.insert(canonical) {
            return Err(format!("duplicate modifier `{canonical}`"));
        }
        modifiers.push(canonical);
    }

    let key = tokens
        .last()
        .ok_or_else(|| "missing key token".to_string())
        .and_then(|token| canonical_key(token))?;

    modifiers.sort_unstable();
    let mut normalized = modifiers.join("+");
    if !normalized.is_empty() {
        normalized.push('+');
    }
    normalized.push_str(&key);

    Ok(normalized)
}

fn canonical_modifier(token: &str) -> Option<&'static str> {
    if token.eq_ignore_ascii_case("ctrl") {
        Some("Ctrl")
    } else if token.eq_ignore_ascii_case("alt") {
        Some("Alt")
    } else if token.eq_ignore_ascii_case("shift") {
        Some("Shift")
    } else if token.eq_ignore_ascii_case("meta") {
        Some("Meta")
    } else {
        None
    }
}

fn canonical_key(token: &str) -> Result<String, String> {
    if token.len() == 1 {
        let ch = token
            .chars()
            .next()
            .expect("single-char token should contain one character");
        if ch.is_ascii_alphanumeric() {
            return Ok(ch.to_ascii_uppercase().to_string());
        }
    }

    let specials: [(&str, &str); 22] = [
        ("enter", "Enter"),
        ("tab", "Tab"),
        ("esc", "Esc"),
        ("backspace", "Backspace"),
        ("left", "Left"),
        ("right", "Right"),
        ("up", "Up"),
        ("down", "Down"),
        ("home", "Home"),
        ("end", "End"),
        ("pageup", "PageUp"),
        ("pagedown", "PageDown"),
        ("delete", "Delete"),
        ("insert", "Insert"),
        ("space", "Space"),
        ("f1", "F1"),
        ("f2", "F2"),
        ("f3", "F3"),
        ("f4", "F4"),
        ("f5", "F5"),
        ("f6", "F6"),
        ("f7", "F7"),
    ];

    if let Some((_, canonical)) = specials
        .iter()
        .find(|(name, _)| token.eq_ignore_ascii_case(name))
    {
        return Ok((*canonical).to_string());
    }

    if ["f8", "f9", "f10", "f11", "f12"]
        .iter()
        .any(|name| token.eq_ignore_ascii_case(name))
    {
        return Ok(token.to_ascii_uppercase());
    }

    Err(format!("unsupported key `{token}`"))
}

fn deserialize_keybindings<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct KeybindingsVisitor;

    impl<'de> de::Visitor<'de> for KeybindingsVisitor {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a mapping of command names to keystroke strings")
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            let mut keybindings = BTreeMap::new();
            while let Some((command, keystroke)) = access.next_entry::<String, String>()? {
                if keybindings.contains_key(&command) {
                    return Err(de::Error::custom(format!(
                        "duplicate keybinding command `{command}`"
                    )));
                }
                keybindings.insert(command, keystroke);
            }
            Ok(keybindings)
        }
    }

    deserializer.deserialize_map(KeybindingsVisitor)
}

fn parse_duplicate_keybinding_error(error: &serde_yaml::Error) -> Option<String> {
    let message = error.to_string();
    let prefix = "duplicate keybinding command `";
    if let Some(start) = message.find(prefix) {
        let rest = &message[start + prefix.len()..];
        if let Some(end) = rest.find('`') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::resolve_config_path;
    use std::path::PathBuf;

    #[test]
    fn override_path_wins() {
        let override_path = PathBuf::from("custom-config.yaml");
        let resolved = resolve_config_path(Some(override_path.clone())).expect("should resolve");
        assert_eq!(resolved, override_path);
    }
}
