use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

const LUCIDE_LICENSE: &str = "ISC";
const LUCIDE_SOURCE: &str = "https://github.com/lucide-icons/lucide/tree/main/icons";
const LUCIDE_RAW_BASE: &str = "https://raw.githubusercontent.com/lucide-icons/lucide/main/icons";

struct Icon {
    svg: String,
    license: &'static str,
    source: String,
}

pub(crate) fn icon(
    root: &Path,
    name: &str,
    provider: &str,
    alias: Option<&str>,
    force: bool,
) -> Result<(), String> {
    let provider = provider.to_ascii_lowercase();
    if provider != "lucide" {
        return Err(format!(
            "unsupported icon provider '{provider}'; supported providers: lucide"
        ));
    }

    validate_asset_name(name).map_err(|error| format!("invalid provider icon name: {error}"))?;
    let asset_name = alias.unwrap_or(name);
    validate_asset_name(asset_name).map_err(|error| format!("invalid local icon name: {error}"))?;

    let icon = fetch_lucide_icon(name)?;
    let svg = sanitize_lucide_svg(&icon.svg)?;
    let icons_dir = root.join("ui/icons");
    let provider_dir = icons_dir.join(&provider);
    let destination = provider_dir.join(format!("{asset_name}.svg"));
    fs::create_dir_all(&provider_dir)
        .map_err(|error| format!("cannot create {}: {error}", provider_dir.display()))?;

    let manifest = icons_dir.join("icons.toml");
    let manifest_contents = read_manifest(&manifest)?;
    let existing_entry = manifest_icon(&manifest_contents, asset_name);
    let manifest_has_entry = existing_entry.is_some();
    if (destination.exists() || manifest_has_entry) && !force {
        return Err(format!(
            "icon '{asset_name}' already exists; use --force to replace it"
        ));
    }
    if destination.exists() && !destination.is_file() {
        return Err(format!("{} is not a regular file", destination.display()));
    }

    let record = format!(
        "\n[[icon]]\nname = {}\nprovider = {}\npath = {}\nlicense = {}\nsource = {}\n",
        toml_string(asset_name),
        toml_string(&provider),
        toml_string(&format!("{provider}/{asset_name}.svg")),
        toml_string(icon.license),
        toml_string(&icon.source),
    );

    // Write both outputs through temporary sibling files. If manifest replacement fails,
    // leave the prior SVG in place as well.
    let svg_temp = temp_path(&destination);
    let manifest_temp = temp_path(&manifest);
    write_new_file(&svg_temp, svg.as_bytes())?;
    let mut next_manifest = if force {
        remove_manifest_icon(&manifest_contents, asset_name)
    } else {
        manifest_contents
    };
    next_manifest.push_str(&record);
    if let Err(error) = write_new_file(&manifest_temp, next_manifest.as_bytes()) {
        let _ = fs::remove_file(&svg_temp);
        return Err(error);
    }

    let previous_svg = if destination.exists() {
        let backup = temp_path(&destination).with_extension("svg.backup");
        if let Err(error) = fs::rename(&destination, &backup) {
            let _ = fs::remove_file(&svg_temp);
            let _ = fs::remove_file(&manifest_temp);
            return Err(format!("cannot stage existing icon: {error}"));
        }
        Some(backup)
    } else {
        None
    };

    if let Err(error) = fs::rename(&svg_temp, &destination) {
        let _ = fs::remove_file(&manifest_temp);
        if let Some(backup) = &previous_svg {
            let _ = fs::rename(backup, &destination);
        }
        return Err(format!(
            "cannot install SVG {}: {error}",
            destination.display()
        ));
    }
    if let Err(error) = replace_file(&manifest_temp, &manifest) {
        let _ = fs::remove_file(&destination);
        if let Some(backup) = &previous_svg {
            let _ = fs::rename(backup, &destination);
        }
        return Err(error);
    }
    if let Some(backup) = previous_svg {
        let _ = fs::remove_file(backup);
    }
    if force {
        if let Some((old_provider, old_path)) = existing_entry {
            if old_provider != provider || old_path != format!("{provider}/{asset_name}.svg") {
                let old_asset = icons_dir.join(old_path);
                let _ = fs::remove_file(old_asset);
            }
        }
    }

    println!(
        "Imported {provider} icon '{name}' as '{asset_name}' into {}",
        destination.display()
    );
    Ok(())
}

fn fetch_lucide_icon(name: &str) -> Result<Icon, String> {
    let url = format!("{LUCIDE_RAW_BASE}/{name}.svg");
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(20))
        .user_agent(concat!("Photon CLI/", env!("CARGO_PKG_VERSION")))
        .build();
    let response = agent.get(&url).call().map_err(|error| match error {
        ureq::Error::Status(404, _) => format!("Lucide icon '{name}' was not found"),
        other => format!("failed to fetch Lucide icon '{name}': {other}"),
    })?;
    let svg = response
        .into_string()
        .map_err(|error| format!("Lucide returned a non-text response: {error}"))?;
    if svg.len() > 1_000_000 {
        return Err("downloaded SVG exceeds the 1 MB size limit".into());
    }
    Ok(Icon {
        svg,
        license: LUCIDE_LICENSE,
        source: format!("{LUCIDE_SOURCE}/{name}.svg"),
    })
}

fn sanitize_lucide_svg(svg: &str) -> Result<String, String> {
    let trimmed = svg.trim();
    if !trimmed.starts_with("<svg") || !trimmed.ends_with("</svg>") {
        return Err("provider response is not a complete SVG document".into());
    }
    let root_end = trimmed
        .find('>')
        .ok_or_else(|| "provider SVG has no root element".to_owned())?;
    if trimmed[..root_end].contains("<!") {
        return Err("provider SVG contains a declaration".into());
    }
    let attributes = parse_xml_attributes(&trimmed[4..root_end])?;
    if attributes.iter().any(|(key, _)| {
        ![
            "xmlns",
            "viewBox",
            "width",
            "height",
            "fill",
            "stroke",
            "stroke-width",
            "stroke-linecap",
            "stroke-linejoin",
        ]
        .contains(&key.as_str())
    }) {
        return Err("provider SVG contains an unsupported root attribute".into());
    }
    let view_box = attributes
        .iter()
        .find(|(name, _)| name == "viewBox")
        .map(|(_, value)| value.clone())
        .ok_or_else(|| "provider SVG is missing viewBox".to_owned())?;
    if view_box.chars().any(char::is_control) || view_box.len() > 64 {
        return Err("provider SVG has an invalid viewBox".into());
    }
    let content_start = root_end + 1;
    let content_end = trimmed.len() - "</svg>".len();
    let elements = parse_lucide_elements(&trimmed[content_start..content_end])?;
    let mut normalized = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\">",
        escape_xml_attribute(&view_box)
    );
    for element in elements {
        normalized.push_str("\n  ");
        normalized.push_str(&element);
    }
    normalized.push('\n');
    normalized.push_str("</svg>\n");
    Ok(normalized)
}

fn parse_xml_attributes(input: &str) -> Result<Vec<(String, String)>, String> {
    let mut attributes = Vec::new();
    let mut remaining = input.trim();
    while !remaining.is_empty() {
        let equals = remaining
            .find('=')
            .ok_or_else(|| "provider SVG has malformed attributes".to_owned())?;
        let name = remaining[..equals].trim();
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            return Err("provider SVG has malformed attributes".into());
        }
        let value_start = remaining[equals + 1..].trim_start();
        let value_start = value_start
            .strip_prefix('"')
            .ok_or_else(|| "provider SVG attributes must use quoted values".to_owned())?;
        let value_end = value_start
            .find('"')
            .ok_or_else(|| "provider SVG has an unterminated attribute".to_owned())?;
        if attributes.iter().any(|(existing, _)| existing == name) {
            return Err(format!("provider SVG repeats attribute '{name}'"));
        }
        attributes.push((name.to_owned(), value_start[..value_end].to_owned()));
        remaining = value_start[value_end + 1..].trim_start();
    }
    Ok(attributes)
}

fn parse_lucide_elements(input: &str) -> Result<Vec<String>, String> {
    let mut elements = Vec::new();
    let mut remaining = input.trim();
    while !remaining.is_empty() {
        let (element_name, element_start) = if let Some(start) = remaining.strip_prefix("<path ") {
            ("path", start)
        } else if let Some(start) = remaining.strip_prefix("<rect ") {
            ("rect", start)
        } else {
            return Err("Lucide SVG contains unsupported elements".into());
        };
        let element_end = element_start
            .find("/>")
            .ok_or_else(|| format!("Lucide SVG contains a malformed {element_name}"))?;
        let attributes = parse_xml_attributes(&element_start[..element_end])?;
        let element = match element_name {
            "path" => {
                if attributes.len() != 1 || attributes[0].0 != "d" {
                    return Err("Lucide SVG path has unexpected attributes".into());
                }
                let path_data = &attributes[0].1;
                if path_data.is_empty() || path_data.len() > 100_000 {
                    return Err("Lucide SVG path data is empty or too large".into());
                }
                format!("<path d=\"{}\" />", escape_xml_attribute(path_data))
            }
            "rect" => sanitize_lucide_rect(&attributes)?,
            _ => unreachable!(),
        };
        elements.push(element);
        remaining = element_start[element_end + 2..].trim_start();
    }
    if elements.is_empty() {
        return Err("Lucide SVG has no supported elements".into());
    }
    Ok(elements)
}

fn sanitize_lucide_rect(attributes: &[(String, String)]) -> Result<String, String> {
    if attributes
        .iter()
        .any(|(name, _)| !["x", "y", "width", "height", "rx", "ry"].contains(&name.as_str()))
    {
        return Err("Lucide SVG rectangle has unexpected attributes".into());
    }
    let number = |name: &str, default: f64| -> Result<f64, String> {
        let Some((_, value)) = attributes.iter().find(|(attribute, _)| attribute == name) else {
            return Ok(default);
        };
        let value = value
            .parse::<f64>()
            .map_err(|_| format!("Lucide SVG rectangle has an invalid {name}"))?;
        if !value.is_finite() {
            return Err(format!("Lucide SVG rectangle has an invalid {name}"));
        }
        Ok(value)
    };
    let x = number("x", 0.0)?;
    let y = number("y", 0.0)?;
    let width = number("width", 0.0)?;
    let height = number("height", 0.0)?;
    let rx = number("rx", number("ry", 0.0)?)?;
    let ry = number("ry", rx)?;
    if width <= 0.0 || height <= 0.0 || rx < 0.0 || ry < 0.0 {
        return Err("Lucide SVG rectangle has invalid dimensions".into());
    }
    Ok(format!(
        "<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" rx=\"{rx}\" ry=\"{ry}\" />"
    ))
}

fn escape_xml_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn validate_asset_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 80
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || name.starts_with('-')
        || name.ends_with('-')
    {
        return Err("use lowercase letters, digits, and internal hyphens only".into());
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

fn manifest_icon(contents: &str, name: &str) -> Option<(String, String)> {
    contents.split("[[icon]]").skip(1).find_map(|entry| {
        let field = |key: &str| {
            entry.lines().find_map(|line| {
                let (field, value) = line.trim().split_once('=')?;
                (field.trim() == key).then(|| value.trim().trim_matches('"').to_owned())
            })
        };
        if field("name").as_deref() != Some(name) {
            return None;
        }
        Some((field("provider")?, field("path")?))
    })
}

fn remove_manifest_icon(contents: &str, name: &str) -> String {
    let mut result = String::new();
    let mut current = String::new();
    for line in contents.lines() {
        if line.trim() == "[[icon]]" {
            if !current.is_empty() && !manifest_entry_named(&current, name) {
                result.push_str(&current);
            }
            current.clear();
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.is_empty() && !manifest_entry_named(&current, name) {
        result.push_str(&current);
    }
    result
}

fn manifest_entry_named(entry: &str, name: &str) -> bool {
    entry
        .lines()
        .any(|line| line.trim() == format!("name = {}", toml_string(name)))
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn temp_path(path: &Path) -> PathBuf {
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}

fn write_new_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    if let Err(error) = file.write_all(contents) {
        let _ = fs::remove_file(path);
        return Err(format!("cannot write {}: {error}", path.display()));
    }
    Ok(())
}

fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
    #[cfg(windows)]
    if to.exists() {
        fs::remove_file(to).map_err(|error| format!("cannot replace {}: {error}", to.display()))?;
    }
    fs::rename(from, to).map_err(|error| format!("cannot update {}: {error}", to.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizer_normalizes_lucide_root_attributes() {
        let svg = r#"<svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M5 12h14"/></svg>"#;
        let normalized = sanitize_lucide_svg(svg).unwrap();
        assert!(
            normalized
                .starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"")
        );
        assert!(!normalized.contains("width=\"24\""));
        assert!(normalized.contains("<path d=\"M5 12h14\" />"));
    }

    #[test]
    fn sanitizer_rejects_active_or_external_content() {
        let script = r#"<svg viewBox="0 0 24 24"><script/></svg>"#;
        let reference =
            r#"<svg viewBox="0 0 24 24"><use href="https://example.com/icon.svg"/></svg>"#;
        assert!(sanitize_lucide_svg(script).is_err());
        assert!(sanitize_lucide_svg(reference).is_err());
    }

    #[test]
    fn manifest_replacement_only_removes_matching_icon() {
        let existing = "[[icon]]\nname = \"close\"\nprovider = \"lucide\"\npath = \"lucide/close.svg\"\n\n[[icon]]\nname = \"minimize\"\nprovider = \"lucide\"\npath = \"lucide/minimize.svg\"\n";
        let replaced = remove_manifest_icon(existing, "close");
        assert!(manifest_icon(&replaced, "close").is_none());
        assert_eq!(
            manifest_icon(&replaced, "minimize"),
            Some(("lucide".into(), "lucide/minimize.svg".into()))
        );
    }
}
