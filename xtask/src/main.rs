use std::{env, error::Error, fs, path::Path, process::Command};

use time::{format_description::FormatItem, macros::format_description, OffsetDateTime};

type DynError = Box<dyn Error>;

fn main() -> Result<(), DynError> {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        Some("update-rpm-changelog") => {
            let spec_path = args
                .next()
                .unwrap_or_else(|| "packaging/rpm/moove.spec".to_owned());
            update_rpm_changelog(Path::new(&spec_path))
        }
        Some(command) => Err(format!("unknown xtask command: {command}").into()),
        None => Err("missing xtask command".into()),
    }
}

fn update_rpm_changelog(spec_path: &Path) -> Result<(), DynError> {
    let spec = fs::read_to_string(spec_path)?;
    let version = find_version(&spec)?;
    let (name, email) = git_identity()?;
    let header = format!("* {} {} <{}> - {}-1", rpm_date()?, name, email, version);
    let body = format!("- Release v{version}");

    let mut lines: Vec<&str> = spec.lines().collect();
    let changelog_index = lines
        .iter()
        .position(|line| *line == "%changelog")
        .ok_or("%changelog section not found")?;
    let first_entry_index = changelog_index + 1;

    if lines
        .get(first_entry_index)
        .is_some_and(|line| line.ends_with(&format!(" - {version}-1")))
    {
        return Ok(());
    }

    lines.insert(first_entry_index, &body);
    lines.insert(first_entry_index, &header);

    fs::write(spec_path, format!("{}\n", lines.join("\n")))?;
    Ok(())
}

fn find_version(spec: &str) -> Result<String, DynError> {
    spec.lines()
        .find_map(|line| line.strip_prefix("Version:").map(str::trim))
        .filter(|version| !version.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Version line not found".into())
}

fn git_identity() -> Result<(String, String), DynError> {
    let name = git_config("user.name")?;
    let email = git_config("user.email")?;

    if name.is_empty() {
        return Err("git config user.name is empty".into());
    }
    if email.is_empty() {
        return Err("git config user.email is empty".into());
    }

    Ok((name, email))
}

fn git_config(key: &str) -> Result<String, DynError> {
    let output = Command::new("git").args(["config", key]).output()?;

    if !output.status.success() {
        return Err(format!("git config {key} failed").into());
    }

    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn rpm_date() -> Result<String, DynError> {
    const RPM_DATE_FORMAT: &[FormatItem<'_>] =
        format_description!("[weekday repr:short] [month repr:short] [day padding:zero] [year]");

    Ok(OffsetDateTime::now_utc().format(RPM_DATE_FORMAT)?)
}
