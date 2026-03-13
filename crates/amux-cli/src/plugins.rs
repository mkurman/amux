use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PLUGINS_DIR: &str = "plugins";
const REGISTRY_FILE: &str = "registry.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPluginRecord {
    pub package_name: String,
    pub package_version: String,
    pub plugin_name: String,
    pub entry_path: String,
    pub format: String,
    pub installed_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PluginRegistry {
    #[serde(default)]
    plugins: Vec<InstalledPluginRecord>,
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, rename = "amuxPlugin")]
    amux_plugin: Option<AmuxPluginManifest>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AmuxPluginManifest {
    EntryPath(String),
    Detailed { entry: String, #[serde(default)] format: Option<String> },
}

impl AmuxPluginManifest {
    fn entry(&self) -> &str {
        match self {
            Self::EntryPath(entry) => entry,
            Self::Detailed { entry, .. } => entry,
        }
    }

    fn format(&self) -> &str {
        match self {
            Self::EntryPath(_) => "script",
            Self::Detailed { format, .. } => format.as_deref().unwrap_or("script"),
        }
    }
}

fn plugins_root() -> Result<PathBuf> {
    Ok(amux_protocol::ensure_amux_data_dir()?.join(PLUGINS_DIR))
}

fn registry_path() -> Result<PathBuf> {
    Ok(plugins_root()?.join(REGISTRY_FILE))
}

fn ensure_plugin_workspace(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root)?;

    let package_json_path = root.join("package.json");
    if !package_json_path.exists() {
        let content = serde_json::json!({
            "name": "amux-external-plugins",
            "private": true,
            "description": "Runtime-installed amux plugins"
        });
        std::fs::write(package_json_path, serde_json::to_vec_pretty(&content)?)?;
    }

    Ok(())
}

fn npm_command() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

fn package_name_from_spec(spec: &str) -> Result<String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        bail!("plugin package spec cannot be empty");
    }

    let candidate_path = Path::new(trimmed);
    if candidate_path.exists() {
        let package_json_path = if candidate_path.is_dir() {
            candidate_path.join("package.json")
        } else {
            bail!("local plugin installation currently supports package directories, not single files");
        };

        let raw = std::fs::read_to_string(&package_json_path)
            .with_context(|| format!("failed to read {}", package_json_path.display()))?;
        let package_json: PackageJson = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {}", package_json_path.display()))?;
        return Ok(package_json.name);
    }

    if let Some(rest) = trimmed.strip_prefix("npm:") {
        return package_name_from_spec(rest);
    }

    if trimmed.starts_with('@') {
        let slash_idx = trimmed
            .find('/')
            .ok_or_else(|| anyhow!("invalid scoped package spec '{trimmed}'"))?;
        let tail = &trimmed[(slash_idx + 1)..];
        if let Some(version_sep) = tail.rfind('@') {
            if version_sep > 0 {
                return Ok(trimmed[..(slash_idx + 1 + version_sep)].to_string());
            }
        }
        return Ok(trimmed.to_string());
    }

    Ok(trimmed.split('@').next().unwrap_or(trimmed).to_string())
}

fn package_dir(root: &Path, package_name: &str) -> PathBuf {
    let mut dir = root.join("node_modules");
    for part in package_name.split('/') {
        dir = dir.join(part);
    }
    dir
}

fn load_registry() -> Result<PluginRegistry> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(PluginRegistry::default());
    }

    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let registry = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(registry)
}

fn save_registry(registry: &PluginRegistry) -> Result<()> {
    let path = registry_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_vec_pretty(registry)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn validate_plugin_package(package_dir: &Path) -> Result<InstalledPluginRecord> {
    let package_json_path = package_dir.join("package.json");
    let raw = std::fs::read_to_string(&package_json_path)
        .with_context(|| format!("failed to read {}", package_json_path.display()))?;
    let package_json: PackageJson = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", package_json_path.display()))?;

    let manifest = package_json
        .amux_plugin
        .ok_or_else(|| anyhow!("package '{}' is missing the required 'amuxPlugin' field", package_json.name))?;

    let format = manifest.format().trim().to_lowercase();
    if format != "script" {
        bail!(
            "package '{}' declares unsupported amux plugin format '{}'; only 'script' is currently supported",
            package_json.name,
            format
        );
    }

    let package_root = package_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", package_dir.display()))?;
    let entry_path = package_dir.join(manifest.entry());
    if !entry_path.is_file() {
        bail!(
            "package '{}' declares amuxPlugin.entry='{}' but the file does not exist",
            package_json.name,
            manifest.entry()
        );
    }

    let canonical_entry_path = entry_path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", entry_path.display()))?;
    if canonical_entry_path != package_root && !canonical_entry_path.starts_with(&package_root) {
        bail!(
            "package '{}' declares amuxPlugin.entry='{}' outside the installed package directory",
            package_json.name,
            manifest.entry()
        );
    }

    let installed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(InstalledPluginRecord {
        package_name: package_json.name.clone(),
        package_version: package_json.version.unwrap_or_else(|| "0.0.0".to_string()),
        plugin_name: package_json.name,
        entry_path: canonical_entry_path.to_string_lossy().to_string(),
        format,
        installed_at,
    })
}

pub fn install_plugin(package_spec: &str) -> Result<InstalledPluginRecord> {
    let root = plugins_root()?;
    ensure_plugin_workspace(&root)?;

    let package_name = package_name_from_spec(package_spec)?;

    let status = Command::new(npm_command())
        .arg("install")
        .arg("--ignore-scripts")
        .arg("--prefix")
        .arg(&root)
        .arg(package_spec)
        .status()
        .with_context(|| "failed to launch npm; ensure Node.js and npm are installed and on PATH")?;

    if !status.success() {
        bail!("npm install failed for plugin spec '{package_spec}'");
    }

    let installed_dir = package_dir(&root, &package_name);
    if !installed_dir.exists() {
        bail!(
            "npm reported success, but the installed package directory was not found at {}",
            installed_dir.display()
        );
    }

    let installed = validate_plugin_package(&installed_dir)?;
    let mut registry = load_registry()?;
    registry
        .plugins
        .retain(|existing| existing.package_name != installed.package_name);
    registry.plugins.push(installed.clone());
    registry
        .plugins
        .sort_by(|left, right| left.package_name.cmp(&right.package_name));
    save_registry(&registry)?;

    Ok(installed)
}