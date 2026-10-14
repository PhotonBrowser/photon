use std::io::{IsTerminal, Write};

use velocity_browser::{BrowserInstallation, BrowserRegistry};

pub fn resolve<'a>(
    registry: &'a BrowserRegistry,
    selector: &str,
) -> Result<&'a BrowserInstallation, String> {
    if let Ok(number) = selector.parse::<usize>() {
        if number == 0 {
            return Err("browser numbers start at 1".to_owned());
        }
        return registry
            .installations()
            .nth(number - 1)
            .ok_or_else(|| format!("browser number {number} is outside the available list"));
    }

    registry
        .resolve(selector)
        .map_err(|error| error.to_string())
}

pub fn choose<'a>(
    registry: &'a BrowserRegistry,
    selector: Option<&str>,
    allow_multiple: bool,
) -> Result<Vec<&'a BrowserInstallation>, String> {
    if let Some(selector) = selector {
        return resolve(registry, selector).map(|installation| vec![installation]);
    }

    if !std::io::stdin().is_terminal() {
        return Err(
            "no browser selected; use a browser name/ID or number (for example, --browser 1)"
                .to_owned(),
        );
    }

    let installations = registry.installations().collect::<Vec<_>>();
    if installations.is_empty() {
        return Err("no supported browser installations were found".to_owned());
    }

    let mut rows = Vec::with_capacity(installations.len());
    for (index, installation) in installations.iter().enumerate() {
        let family_count = registry.by_browser(&installation.metadata.id).len();
        let name = if family_count > 1 {
            format!(
                "{} [{}]",
                installation.metadata.name, installation.installation_id
            )
        } else {
            installation.metadata.name.clone()
        };
        rows.push(vec![
            (index + 1).to_string(),
            name,
            installation
                .version
                .as_deref()
                .unwrap_or("unknown")
                .to_owned(),
        ]);
    }

    println!("\nSelect a browser:");
    crate::terminal::print_table(&["#", "Browser", "Version"], rows, &[0]);
    if allow_multiple {
        print!("Enter number(s) separated by commas, or 'all': ");
    } else {
        print!("Enter a number: ");
    }
    std::io::stdout()
        .flush()
        .map_err(|error| format!("cannot display browser selection prompt: {error}"))?;

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("cannot read browser selection: {error}"))?;
    let input = input.trim();
    if input.eq_ignore_ascii_case("q") || input.eq_ignore_ascii_case("cancel") {
        return Err("browser selection cancelled".to_owned());
    }
    if input.eq_ignore_ascii_case("all") {
        if allow_multiple {
            return Ok(installations);
        }
        return Err("select one browser number".to_owned());
    }

    let mut selected = Vec::new();
    for value in input.split(',').map(str::trim) {
        let number = value
            .parse::<usize>()
            .map_err(|_| format!("invalid browser selection '{value}'"))?;
        if number == 0 || number > installations.len() {
            return Err(format!(
                "browser number must be between 1 and {}",
                installations.len()
            ));
        }
        let installation = installations[number - 1];
        if selected.iter().any(|current: &&BrowserInstallation| {
            current.installation_id == installation.installation_id
        }) {
            return Err(format!(
                "browser number {number} was selected more than once"
            ));
        }
        selected.push(installation);
    }
    if selected.is_empty() || (!allow_multiple && selected.len() != 1) {
        return Err("select exactly one browser number".to_owned());
    }
    Ok(selected)
}
