use std::fs::Metadata;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use fs_extra::dir::CopyOptions;
use normpath::PathExt;
use regex::Regex;

#[derive(Debug, Parser, Default)]
#[command(version)]
pub struct CommandLine {
    /// Paths or wildcard patterns to move
    #[arg(value_hint = clap::ValueHint::AnyPath)]
    pub paths: Vec<String>,
    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
    /// Sort in natural order
    #[arg(short, long)]
    pub sort: bool,
    /// Treat as absolute paths
    #[arg(short, long)]
    pub absolute: bool,
    /// Directories themselves, not their contents
    #[arg(short, long)]
    pub directory: bool,
    /// Include hidden files
    #[arg(short, long)]
    pub with_hidden: bool,
    /// Exclude regular expression pattern
    #[arg(short, long, value_name = "PATTERN")]
    pub exclude_pattern: Option<Regex>,
    /// Copy without moving
    #[arg(short, long)]
    pub copy: bool,
    /// Dry-run
    #[arg(short = 'u', long)]
    pub dry_run: bool,
    /// Abort in case of collision (prompt as default)
    #[arg(short, long)]
    pub oops: bool,
    /// Overwrite existing files without prompting (error if destination is a directory or symlink to directory)
    #[arg(short, long)]
    pub force: bool,
    /// Overwrite existing directories without prompting; implies --force for files (error if source is a symlink to directory)
    #[arg(short = 'F', long)]
    pub force_dir: bool,
    /// No output to stdout/strerr even if error
    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Debug)]
pub struct Operation {
    pub kind: OperationKind,
    pub src: Source,
    pub dst: Destination,
}

#[derive(Debug)]
pub enum OperationKind {
    Move,
    Copy,
    Remove,
}

#[derive(Debug, Clone)]
pub struct Source {
    pub text: String,
    pub path: PathBuf,
    pub abs: PathBuf,
    pub meta: Metadata,
}

#[derive(Debug, Clone)]
pub struct Destination {
    pub text: String,
    pub path: PathBuf,
}

static SEPARATORS: &[char] = &['/', '\\'];

trait PathUtilExt {
    /// NOTE Can be replaced with `std::path::absolute` in the future.
    fn absolute(&self) -> Result<normpath::BasePathBuf>;
    fn is_hidden(&self) -> Result<bool>;
    fn is_identical(&self, other: &Path) -> bool;
}

impl PathUtilExt for Path {
    fn is_identical(&self, other: &Path) -> bool {
        if cfg!(target_family = "windows") {
            self == other || self.as_os_str().eq_ignore_ascii_case(other.as_os_str())
        } else {
            self == other
        }
    }

    #[cfg(target_family = "windows")]
    fn absolute(&self) -> Result<normpath::BasePathBuf> {
        self.normalize_virtually().with_context(|| {
            format!(
                "Failed to normalize path. {}",
                self.to_string_lossy().yellow().underline()
            )
        })
    }

    #[cfg(target_family = "unix")]
    fn absolute(&self) -> Result<normpath::BasePathBuf> {
        self.normalize().with_context(|| {
            format!(
                "Failed to normalize path. {}",
                self.to_string_lossy().yellow().underline()
            )
        })
    }

    #[cfg(target_family = "windows")]
    fn is_hidden(&self) -> Result<bool> {
        use std::os::windows::prelude::*;
        let metadata = std::fs::metadata(self).with_context(|| {
            format!(
                "Failed to read metadata of {}",
                self.to_string_lossy().yellow().underline()
            )
        })?;
        Ok((metadata.file_attributes() & 0x2) > 0)
    }

    #[cfg(target_family = "unix")]
    fn is_hidden(&self) -> Result<bool> {
        Ok(self
            .file_name()
            .with_context(|| {
                format!(
                    "Failed to get file name {}",
                    self.to_string_lossy().yellow().underline()
                )
            })?
            .to_string_lossy()
            .starts_with('.'))
    }
}

pub fn try_main(args: &CommandLine) -> Result<usize> {
    let sources = &sources_from(args)?;
    let operations = &operations_from(sources, args)?;
    let mut processed = 0;
    for o in operations.iter() {
        execute_operation(o, args)?;
        if args.dry_run {
            continue;
        }
        processed += 1;
    }
    Ok(processed)
}

pub fn sources_from(args: &CommandLine) -> Result<Vec<Source>> {
    let mut sources: Vec<Source> = Vec::new();
    let paths = list_files(&args.paths)?;
    for p in paths.iter().map(|p| p.trim_end_matches(SEPARATORS)) {
        let path = &PathBuf::from(if cfg!(target_family = "windows") {
            p.replace('/', "\\")
        } else {
            p.to_string()
        });
        let stat = &path.symlink_metadata().with_context(|| {
            format!(
                "Failed to access {}",
                path.to_string_lossy().yellow().underline()
            )
        })?;
        if stat.is_file() || stat.is_symlink() || args.directory {
            put_source(&mut sources, path, args)?;
        } else {
            let mut children = Vec::new();
            for entry in std::fs::read_dir(path).with_context(|| {
                format!(
                    "Failed to list files of directory. {}",
                    path.to_string_lossy().yellow().underline()
                )
            })? {
                children.push(entry?.path());
            }
            children.sort_unstable_by(|a, b| {
                natord::compare(&a.to_string_lossy(), &b.to_string_lossy())
            });
            for child in children {
                put_source(&mut sources, &child, args)?;
            }
            if sources.is_empty() {
                anyhow::bail!(
                    "Directory is empty. {}\n\
                    Use --directory for the directory itself.",
                    path.to_string_lossy().yellow().underline()
                );
            }
        }
    }
    if args.sort {
        sources.sort_unstable_by(|a, b| natord::compare(&a.text, &b.text));
    }
    Ok(sources)
}

pub fn list_files(args: &[String]) -> Result<Vec<String>> {
    use glob::glob;
    let mut paths = Vec::new();
    for arg in args.iter() {
        let mut globbed = Vec::new();
        for path in
            glob(arg).with_context(|| format!("Invalid pattern {}", arg.yellow().underline()))?
        {
            globbed
                .push(path.with_context(|| format!("Failed to glob {}", arg.yellow().underline()))?)
        }
        if globbed.is_empty() {
            anyhow::bail!("Failed to access {}", arg);
        }
        globbed.sort_unstable();
        paths.append(
            &mut globbed
                .iter()
                .map(|g| g.to_string_lossy().to_string())
                .collect(),
        );
    }
    Ok(paths)
}

pub fn put_source(sources: &mut Vec<Source>, path: &Path, args: &CommandLine) -> Result<()> {
    let abs = path.absolute()?;
    let abs = abs.as_path();
    if abs.parent().is_none() {
        anyhow::bail!(
            "Source should not be the root directory. {}",
            path.to_string_lossy().yellow().underline()
        );
    }
    if !args.with_hidden && abs.is_hidden()? {
        return Ok(());
    }
    let new_path = if args.absolute { abs } else { path };
    let new_path_text = new_path
        .to_str()
        .with_context(|| {
            format!(
                "Failed to convert path to UTF-8. {}",
                path.to_string_lossy().to_string().yellow().underline()
            )
        })?
        .trim_end_matches(SEPARATORS)
        .to_string();
    if let Some(pattern) = &args.exclude_pattern {
        if pattern.is_match(&new_path_text) {
            return Ok(());
        }
    }
    let new_src = Source {
        text: new_path_text,
        path: new_path.to_path_buf(),
        abs: abs.to_path_buf(),
        meta: new_path.symlink_metadata().with_context(|| {
            format!(
                "Failed to access {}",
                new_path.to_string_lossy().yellow().underline()
            )
        })?,
    };
    for src in sources.iter() {
        if src.abs.is_identical(&new_src.abs) {
            anyhow::bail!(
                "Duplicated source. {}",
                new_src.abs.to_string_lossy().yellow().underline()
            );
        }
    }
    sources.push(new_src);
    Ok(())
}

pub fn operations_from(sources: &[Source], args: &CommandLine) -> Result<Vec<Operation>> {
    let mut operations = Vec::new();
    let mut text = sources
        .iter()
        .map(|src| {
            let mut line = src.text.to_owned();
            if src.path.is_dir()
                && !src.path.is_symlink()
                && !line.ends_with(std::path::MAIN_SEPARATOR)
            {
                line.push(std::path::MAIN_SEPARATOR);
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    'redo: loop {
        text = edit::edit(&text)?;
        let lines = text
            .split('\n')
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                Some(line.trim_end_matches(SEPARATORS))
            })
            .collect::<Vec<_>>();
        if lines.len() != sources.len() {
            let message = format!(
                "Number of lines {} does not match the original one {}",
                lines.len().to_string().yellow(),
                sources.len().to_string().yellow()
            );
            if !args.oops {
                println!("{}", message);
                if prompt_redo()? {
                    continue 'redo;
                }
                break 'redo;
            }
            anyhow::bail!(message);
        }
        for (src, line) in sources.iter().zip(lines.iter()) {
            let line = line.to_owned();
            let (kind, line) = if line.starts_with("//") {
                (OperationKind::Remove, src.text.as_str())
            } else if args.copy {
                (OperationKind::Copy, line)
            } else {
                (OperationKind::Move, line)
            };
            let line = if cfg!(target_family = "windows") {
                line.replace('/', "\\")
            } else {
                line.to_string()
            };
            let dst_path = PathBuf::from(&line);
            let removing = matches!(kind, OperationKind::Remove);
            if !removing && (dst_path == src.path || dst_path == src.abs) {
                continue;
            }
            let new_operation = Operation {
                kind,
                src: src.to_owned(),
                dst: Destination {
                    text: line.to_owned(),
                    path: dst_path.to_owned(),
                },
            };
            if !removing {
                if let Err(message) = is_operational(&operations, &new_operation, args) {
                    if !args.oops {
                        println!("{}", message);
                        if prompt_redo()? {
                            continue 'redo;
                        }
                        break 'redo;
                    }
                    anyhow::bail!(message);
                }
            }
            operations.push(new_operation);
        }
        break;
    }
    Ok(operations)
}

pub fn prompt_redo() -> Result<bool> {
    let re_abort = Regex::new(r"^a(bort)?$")?;
    let re_edit = Regex::new(r"^e(dit)?$")?;
    loop {
        print!(
            "{}{} or {}{}? > ",
            "E".bold().underline(),
            "dit".bold(),
            "A".bold().underline(),
            "bort".bold()
        );
        std::io::stdout().flush()?;
        let mut ans = String::new();
        std::io::stdin().read_line(&mut ans)?;
        let ans = ans.trim().to_ascii_lowercase();
        if re_abort.is_match(&ans) {
            return Ok(false);
        }
        if ans.is_empty() || re_edit.is_match(&ans) {
            return Ok(true);
        }
    }
}

pub fn is_operational(
    operations: &[Operation],
    new_operation: &Operation,
    args: &CommandLine,
) -> Result<()> {
    let src = &new_operation.src;
    let dst = &new_operation.dst;
    if dst.text.ends_with(std::path::MAIN_SEPARATOR)
        && (src.meta.is_file() || src.meta.is_symlink())
    {
        anyhow::bail!(
            "Missing file name. {} for {}",
            dst.text.yellow().underline(),
            src.text.underline()
        )
    }
    if operations
        .iter()
        .any(|o| o.dst.path.is_identical(&dst.path))
    {
        anyhow::bail!("Duplicated destination. {}", dst.text.yellow().underline());
    }
    if operations
        .iter()
        .any(|o| o.dst.path.ancestors().any(|a| a.is_identical(&dst.path)))
    {
        anyhow::bail!(
            "Destination should not be included in other destination. {}",
            dst.text.yellow().underline()
        );
    }
    // --force / --force-dir: error if src is a symlink to a directory (unpredictable behavior with fs_extra)
    if (args.force || args.force_dir) && src.path.is_symlink() && src.path.is_dir() {
        anyhow::bail!(
            "Source is a symlink to directory; cannot use --force/--force-dir. {}",
            src.text.yellow().underline()
        )
    }
    // --force: error if src is a directory
    if args.force && !args.force_dir && src.meta.is_dir() {
        anyhow::bail!(
            "Source is a directory; cannot use --force. {}",
            src.text.yellow().underline()
        )
    }
    if dst.path.exists() {
        if args.force_dir {
            // --force-dir: allow overwrite of anything except symlink-to-dir on src (handled above)
        } else if args.force {
            // --force: error if dst is a directory or a symlink to a directory
            if dst.path.is_dir() || (dst.path.is_symlink() && dst.path.is_dir()) {
                anyhow::bail!(
                    "Destination is a directory or symlink to directory; cannot overwrite with --force. {}",
                    dst.text.yellow().underline()
                )
            }
            // Regular file or symlink to file: allow overwrite (dst will be removed before the operation)
        } else {
            anyhow::bail!("Destination exists. {}", dst.text.yellow().underline())
        }
    }
    if dst.path.ancestors().any(|a| {
        // When --force or --force-dir is set, dst itself is allowed to be overwritten
        if (args.force || args.force_dir) && a == dst.path {
            return false;
        }
        if !a.exists() {
            false
        } else if a.is_file() {
            true
        } else if a.is_symlink() {
            if let Ok(p) = a.read_link() {
                p.is_file()
            } else {
                false
            }
        } else {
            false
        }
    }) {
        anyhow::bail!(
            "Ancestor of destination should not be a file.\n\
             Destination: {}",
            dst.text.yellow().underline()
        );
    }
    Ok(())
}

pub fn execute_operation(o: &Operation, args: &CommandLine) -> Result<()> {
    match o.kind {
        OperationKind::Move => {
            if !args.quiet && (args.verbose || args.dry_run) {
                let overwrite = (args.force || args.force_dir) && o.dst.path.exists();
                println!(
                    "{} {}{}{}{}",
                    "Move".dimmed(),
                    o.src.text.dimmed().underline(),
                    " → ".dimmed(),
                    o.dst.text.dimmed().underline(),
                    if overwrite {
                        " (overwrite)".dimmed()
                    } else {
                        "".dimmed()
                    }
                );
            }
            if args.dry_run {
                return Ok(());
            }
            execute_move_or_copy(o, args)?;
            if !args.quiet {
                println!(
                    "{} → {}",
                    o.src.text.green().underline(),
                    o.dst.text.green().underline()
                );
            }
        }
        OperationKind::Copy => {
            if !args.quiet && (args.verbose || args.dry_run) {
                let overwrite = (args.force || args.force_dir) && o.dst.path.exists();
                println!(
                    "{} {}{}{}{}",
                    "Copy".dimmed(),
                    o.src.text.dimmed().underline(),
                    " → ".dimmed(),
                    o.dst.text.dimmed().underline(),
                    if overwrite {
                        " (overwrite)".dimmed()
                    } else {
                        "".dimmed()
                    }
                );
            }
            if args.dry_run {
                return Ok(());
            }
            execute_move_or_copy(o, args)?;
            if !args.quiet {
                println!(
                    "{} → {}",
                    o.src.text.green().underline(),
                    o.dst.text.green().underline()
                );
            }
        }
        OperationKind::Remove => {
            if !args.quiet && (args.verbose || args.dry_run) {
                println!("{} {}", "Remove".dimmed(), o.src.text.dimmed().underline());
            }
            if args.dry_run {
                return Ok(());
            }
            execute_remove(o, args)?;
            if !args.quiet {
                println!("Removed {}", o.src.text.green().underline());
            }
        }
    };
    Ok(())
}

pub fn execute_move_or_copy(operation: &Operation, args: &CommandLine) -> Result<()> {
    let Operation { kind, src, dst, .. } = operation;
    let moving = matches!(kind, OperationKind::Move);
    // --force / --force-dir: remove the existing destination before moving/copying
    if (args.force || args.force_dir) && dst.path.exists() {
        if args.force_dir && dst.path.is_dir() {
            if moving {
                // move: remove dst directory entirely, then move src into place
                std::fs::remove_dir_all(&dst.path).with_context(|| {
                    format!(
                        "Failed to remove existing destination directory {}",
                        dst.text.yellow().underline()
                    )
                })?;
            }
            // copy: leave dst directory in place and let fs_extra merge with overwrite=true
        } else {
            // dst is a file or a symlink (to file or to directory): remove it
            std::fs::remove_file(&dst.path).with_context(|| {
                format!(
                    "Failed to remove existing destination {}",
                    dst.text.yellow().underline()
                )
            })?;
        }
    }
    let dst_parent = create_dir(dst, args)?;
    if should_relocate(&src.path, &dst_parent) {
        if !args.quiet && args.verbose {
            println!(
                "{} {} {}",
                if moving { "Moving" } else { "Copying" }.dimmed(),
                src.abs.to_string_lossy().dimmed().underline(),
                dst_parent.to_string_lossy().dimmed().underline()
            );
        }
        if moving {
            fs_extra::move_items(&[&src.path], &dst_parent, &CopyOptions::default()).with_context(
                || {
                    format!(
                        "Failed to move {} to {}",
                        src.text.yellow().underline(),
                        dst_parent.to_string_lossy().yellow().underline()
                    )
                },
            )?;
        } else {
            let copy_options = if args.force_dir {
                CopyOptions { overwrite: true, ..CopyOptions::default() }
            } else {
                CopyOptions::default()
            };
            fs_extra::copy_items(&[&src.path], &dst_parent, &copy_options).with_context(
                || {
                    format!(
                        "Failed to copy {} to {}",
                        src.text.yellow().underline(),
                        dst_parent.to_string_lossy().yellow().underline()
                    )
                },
            )?;
        }
    }
    // Rename if its file name need to be changed.
    // NOTE Can be unwrapped safely, `src` and `dst` cannot be root nor `..`.
    let src_basename = src.path.file_name().unwrap();
    let dst_basename = dst.path.file_name().unwrap();
    if src_basename != dst_basename {
        let from = &dst_parent.join(src_basename);
        let to = &dst_parent.join(dst_basename);
        // --force-dir copy merge: dst (to) still exists as a non-empty directory,
        // so rename would fail on Windows. Instead, merge from into to using
        // fs_extra::dir::copy with overwrite, then remove from.
        if args.force_dir && !moving && to.is_dir() {
            if !args.quiet && args.verbose {
                println!(
                    "{} {}{}{}",
                    "Merging".dimmed(),
                    from.to_string_lossy().dimmed().underline(),
                    " → ".dimmed(),
                    to.to_string_lossy().dimmed().underline()
                );
            }
            fs_extra::dir::copy(
                from,
                to,
                &fs_extra::dir::CopyOptions {
                    overwrite: true,
                    content_only: true,
                    ..fs_extra::dir::CopyOptions::default()
                },
            ).with_context(|| {
                format!(
                    "Failed to merge {} into {}",
                    from.to_string_lossy().yellow().underline(),
                    to.to_string_lossy().yellow().underline()
                )
            })?;
            // Remove the intermediate copy only when it was created by relocation
            // (i.e. from != src.path). When should_relocate was false, from IS src.path
            // and must not be deleted on copy.
            if from != &src.path {
                std::fs::remove_dir_all(from).with_context(|| {
                    format!(
                        "Failed to remove temporary directory {}",
                        from.to_string_lossy().yellow().underline()
                    )
                })?;
            }
        } else {
            if !args.quiet && args.verbose {
                println!(
                    "{} {}{}{}",
                    "Renaming".dimmed(),
                    from.to_string_lossy().dimmed().underline(),
                    " → ".dimmed(),
                    to.to_string_lossy().dimmed().underline()
                );
            }
            // Destination is never over-written, ensured when the operation was made.
            std::fs::rename(from, to).with_context(|| {
                format!(
                    "Failed to rename {} to {}",
                    from.to_string_lossy().yellow().underline(),
                    to.to_string_lossy().yellow().underline()
                )
            })?;
        }
    }
    Ok(())
}

/// Create parent directory if missing.
pub fn create_dir(dst: &Destination, args: &CommandLine) -> Result<PathBuf> {
    let current_dir = std::env::current_dir().context("Failed to get current directory.")?;
    let dst_parent = if dst.text.contains(std::path::MAIN_SEPARATOR) {
        dst.path.parent().unwrap()
    } else {
        &current_dir
    };
    if !dst_parent.exists() {
        if !args.quiet && args.verbose {
            println!(
                "{} {}",
                "Creating directory".dimmed(),
                dst_parent.to_string_lossy().dimmed().underline()
            );
        }
        std::fs::create_dir_all(dst_parent).with_context(|| {
            format!(
                "Failed to create directory. {}",
                dst_parent.to_string_lossy().yellow().underline()
            )
        })?;
    }
    Ok(dst_parent.to_path_buf())
}

pub fn should_relocate(src: &Path, dst_parent: &Path) -> bool {
    // NOTE `Path.parent()` returns `Some("")` in case of simple relative path.
    if let Some(src_parent) = src.parent() {
        !src_parent.as_os_str().is_empty() && src_parent != dst_parent
    } else {
        false
    }
}

pub fn execute_remove(operation: &Operation, _args: &CommandLine) -> Result<()> {
    let path = &operation.src.abs;
    remove_path(path).with_context(|| {
        format!(
            "Failed to remove {}",
            path.to_string_lossy().yellow().underline()
        )
    })?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

#[cfg(not(target_os = "macos"))]
fn remove_path(path: &Path) -> std::result::Result<(), trash::Error> {
    trash::delete(path)
}

#[cfg(test)]
mod lib {
    use std::path::PathBuf;

    use anyhow::{Context, Result};
    use colored::*;
    use normpath::PathExt;

    use super::*;

    /// Create temporary files before starting tests and removed by RAII.
    struct Setup {
        sandbox: PathBuf,
        args: CommandLine,
    }

    impl Setup {
        ///
        /// Create following tree.
        ///
        /// ```ignore
        /// {sandbox}/
        ///   1/
        ///   ├─1.txt
        ///   ├─11/
        ///   │  └─11.txt
        ///   ├─12/
        ///   │  └─12.txt
        ///   2/
        ///   ├─2.txt
        ///   ├─21/
        ///   │  ├─21.txt
        ///   │  └─211/
        ///   │      └─211.txt
        ///   └─22
        ///       └─22.txt
        /// ```
        fn init(key: &str) -> Result<Setup> {
            let sandbox = &std::env::temp_dir().join("moove").join("test").join(key);
            std::fs::create_dir_all(sandbox)?;
            let dirs: Vec<PathBuf> = vec!["1", "1/11", "1/12", "2", "2/21", "2/21/211", "2/22"]
                .iter()
                .map(|d| {
                    sandbox.join(PathBuf::from(if cfg!(target_family = "windows") {
                        d.replace('/', "\\")
                    } else {
                        d.to_string()
                    }))
                })
                .collect();
            for dir in dirs.iter() {
                println!("{} {}", "Creating".dimmed(), dir.to_string_lossy().dimmed());
                std::fs::create_dir_all(dir)?;
            }
            let files: Vec<PathBuf> = dirs
                .iter()
                .map(|dir| dir.join(dir.file_name().unwrap()).with_extension("txt"))
                .collect();
            for file in files.iter() {
                println!(
                    "{} {}",
                    "Creating".dimmed(),
                    file.to_string_lossy().dimmed()
                );
                std::fs::File::create(file)?;
            }
            let setup = Setup {
                sandbox: sandbox.to_owned(),
                args: CommandLine {
                    verbose: true,
                    ..CommandLine::default()
                },
            };
            Ok(setup)
        }

        fn source_from(&self, s: &str) -> Source {
            let path = self.sandbox.join(if cfg!(target_family = "windows") {
                s.replace('/', "\\")
            } else {
                s.to_string()
            });
            Source {
                text: path.to_string_lossy().to_string(),
                path: path.to_owned(),
                abs: path
                    .normalize()
                    .context(format!("Failed to normalize {:?}", path))
                    .unwrap()
                    .into_path_buf(),
                meta: path
                    .metadata()
                    .context(format!("Failed to get metadata {:?}", path))
                    .unwrap(),
            }
        }

        fn destination_from(&self, s: &str) -> Destination {
            let path = self.sandbox.join(if cfg!(target_family = "windows") {
                s.replace('/', "\\")
            } else {
                s.to_string()
            });
            Destination {
                text: path.to_string_lossy().to_string(),
                path: path.to_owned(),
            }
        }

        fn operation_from(&self, src: &str, dst: &str) -> Operation {
            Operation {
                kind: OperationKind::Move,
                src: self.source_from(src),
                dst: self.destination_from(dst),
            }
        }
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            println!(
                "{} {}",
                "Removing".dimmed(),
                self.sandbox.to_string_lossy().dimmed()
            );
            std::fs::remove_dir_all(&self.sandbox).unwrap();
        }
    }

    #[test]
    fn list_sources_normally() -> Result<()> {
        let mut setup = Setup::init("list_sources_normally")?;
        setup
            .args
            .paths
            .push(setup.sandbox.join("1").to_string_lossy().to_string());
        let sources = sources_from(&setup.args)?;
        assert_eq!(sources[0].path, setup.sandbox.join("1/1.txt"));
        assert_eq!(sources[1].path, setup.sandbox.join("1/11"));
        assert_eq!(sources[2].path, setup.sandbox.join("1/12"));
        setup.args.paths.clear();
        setup.args.paths.push(
            setup
                .sandbox
                .join("..")
                .join(&setup.sandbox)
                .join("1")
                .to_string_lossy()
                .to_string(),
        );
        let sources = sources_from(&setup.args)?;
        assert_eq!(sources[0].path, setup.sandbox.join("1/1.txt"));
        assert_eq!(sources[1].path, setup.sandbox.join("1/11"));
        assert_eq!(sources[2].path, setup.sandbox.join("1/12"));
        Ok(())
    }

    #[test]
    fn should_fail_to_list_sources() -> Result<()> {
        let mut setup = Setup::init("should_fail_to_list_sources")?;
        setup.args.paths.push(
            setup
                .sandbox
                .join("does not exist")
                .to_string_lossy()
                .to_string(),
        );
        assert!(sources_from(&setup.args).is_err());
        setup.args.paths.clear();
        setup.args.paths.push("/".to_owned());
        assert!(sources_from(&setup.args).is_err());
        setup.args.paths.clear();
        setup
            .args
            .paths
            .push(setup.sandbox.join("1").to_string_lossy().to_string());
        setup
            .args
            .paths
            .push(setup.args.paths.last().unwrap().clone());
        assert!(sources_from(&setup.args).is_err());
        Ok(())
    }

    #[test]
    fn operate_normally() -> Result<()> {
        let setup = &Setup::init("operate_normally")?;
        let mut operations = Vec::new();
        let new_operation = setup.operation_from("1/11/11.txt", "1/12/moved-11.txt");
        is_operational(&operations, &new_operation, &setup.args)?;
        operations.push(new_operation);
        let new_operation = setup.operation_from("1/12/12.txt", "1/11/moved-12.txt");
        is_operational(&operations, &new_operation, &setup.args)?;
        operations.push(new_operation);
        let new_operation = setup.operation_from("1/1.txt", "1/11/moved-1.txt");
        is_operational(&operations, &new_operation, &setup.args)?;
        operations.push(new_operation);
        let new_operation = setup.operation_from("2/21/211", "moved-211");
        is_operational(&operations, &new_operation, &setup.args)?;
        operations.push(new_operation);
        let new_operation = setup.operation_from("2/22", "moved-211/moved-22");
        is_operational(&operations, &new_operation, &setup.args)?;
        operations.push(new_operation);
        for o in operations.iter() {
            execute_operation(o, &setup.args)?;
        }
        Ok(())
    }

    #[test]
    fn should_not_be_operational() -> Result<()> {
        let setup = &Setup::init("should_not_be_operational")?;
        let operations = vec![
            setup.operation_from("1/11/11.txt", "1/12/moved-11.txt"),
            setup.operation_from("1/12/12.txt", "1/11/moved-12.txt"),
            setup.operation_from("1/1.txt", "1/11/moved-1.txt"),
            setup.operation_from("2/21/211", "moved-211"),
            setup.operation_from("2/22", "moved-211/moved-22"),
        ];
        [
            ("1/11/11.txt", "1/11/11.txt"),
            ("1/11/11.txt", "1/12/12.txt"),
            ("1/11", "2/21/211"),
            ("1/11", "moved-211"),
        ]
        .iter()
        .for_each(|(src, dst)| {
            assert!(
                is_operational(&operations, &setup.operation_from(src, dst), &setup.args).is_err()
            );
        });
        Ok(())
    }

    #[test]
    fn rename_file() -> Result<()> {
        let setup = &Setup::init("rename_file")?;
        let operation = &setup.operation_from("1/11/11.txt", "1/11/renamed-11.txt");
        execute_move_or_copy(operation, &setup.args)?;
        assert!(operation.dst.path.is_file());
        assert!(!operation.src.path.is_file());
        Ok(())
    }

    #[test]
    fn rename_dir() -> Result<()> {
        let setup = &Setup::init("rename_dir")?;
        let operation = &setup.operation_from("1/11", "1/renamed-11");
        execute_move_or_copy(operation, &setup.args)?;
        assert!(operation.dst.path.is_dir());
        assert!(!operation.src.path.is_dir());
        Ok(())
    }

    #[test]
    fn rename_dir_with_sub_dirs() -> Result<()> {
        let setup = &Setup::init("rename_dir_with_sub_dirs")?;
        let operation = &setup.operation_from("1", "renamed-1");
        execute_move_or_copy(operation, &setup.args)?;
        assert!(operation.dst.path.is_dir());
        assert!(!operation.src.path.is_dir());
        Ok(())
    }

    #[test]
    fn move_and_rename_file() -> Result<()> {
        let setup = &Setup::init("move_and_rename_file")?;
        let operation = &setup.operation_from("2/21/211/211.txt", "1/renamed-211.txt");
        execute_move_or_copy(operation, &setup.args)?;
        assert!(operation.dst.path.is_file());
        assert!(!operation.src.path.is_file());
        Ok(())
    }

    #[test]
    fn move_and_rename_directory() -> Result<()> {
        let setup = &Setup::init("move_and_rename_directory")?;
        let operation = &setup.operation_from("2/22", "1/3");
        execute_move_or_copy(operation, &setup.args)?;
        assert!(operation.dst.path.is_dir());
        assert!(!operation.src.path.is_dir());
        Ok(())
    }

    #[test]
    fn dry_run() -> Result<()> {
        let mut setup = Setup::init("dry_run")?;
        setup.args.dry_run = true;
        let operation = setup.operation_from("2/22", "1/3");
        execute_operation(&operation, &setup.args)?;
        assert!(operation.src.path.is_dir());
        assert!(!operation.dst.path.is_dir());
        Ok(())
    }


    // --force-dir: overwrites an existing directory on move (dst is removed and replaced)
    #[test]
    fn force_dir_move_overwrites_existing_directory() -> Result<()> {
        let mut setup = Setup::init("force_dir_move_overwrites_existing_directory")?;
        setup.args.force_dir = true;
        // src: 1/11 (directory with 11.txt), dst: 1/12 (existing directory with 12.txt)
        let operation = setup.operation_from("1/11", "1/12");
        is_operational(&[], &operation, &setup.args)?;
        execute_move_or_copy(&operation, &setup.args)?;
        // dst now contains src's contents (11.txt), 12.txt is gone
        assert!(setup.sandbox.join("1/12").is_dir());
        assert!(setup.sandbox.join("1/12/11.txt").is_file());
        assert!(!setup.sandbox.join("1/12/12.txt").exists());
        assert!(!setup.sandbox.join("1/11").exists());
        Ok(())
    }

    // --force-dir: merges into an existing directory on copy
    #[test]
    fn force_dir_copy_merges_into_existing_directory() -> Result<()> {
        let mut setup = Setup::init("force_dir_copy_merges_into_existing_directory")?;
        setup.args.force_dir = true;
        setup.args.copy = true;
        // src: 1/11 (directory with 11.txt), dst: 1/12 (existing directory with 12.txt)
        let base = setup.operation_from("1/11", "1/12");
        let operation = Operation {
            kind: OperationKind::Copy,
            src: base.src,
            dst: base.dst,
        };
        is_operational(&[], &operation, &setup.args)?;
        execute_move_or_copy(&operation, &setup.args)?;
        // dst retains 12.txt and gains 11.txt (merge)
        assert!(setup.sandbox.join("1/12/12.txt").is_file());
        assert!(setup.sandbox.join("1/12/11.txt").is_file());
        // src is preserved on copy
        assert!(setup.sandbox.join("1/11").is_dir());
        Ok(())
    }

    // --force-dir: also overwrites existing files (implies --force)
    #[test]
    fn force_dir_move_overwrites_existing_file() -> Result<()> {
        let mut setup = Setup::init("force_dir_move_overwrites_existing_file")?;
        setup.args.force_dir = true;
        // src: 1/11/11.txt (file), dst: 1/12/12.txt (existing file)
        let operation = setup.operation_from("1/11/11.txt", "1/12/12.txt");
        is_operational(&[], &operation, &setup.args)?;
        execute_move_or_copy(&operation, &setup.args)?;
        assert!(operation.dst.path.is_file());
        assert!(!operation.src.path.exists());
        Ok(())
    }

    // --force-dir: error if src is a directory without --force-dir
    #[test]
    fn without_force_dir_existing_dst_directory_is_error() -> Result<()> {
        let setup = Setup::init("without_force_dir_existing_dst_directory_is_error")?;
        // src: 1/11 (directory), dst: 1/12 (existing directory)
        let operation = setup.operation_from("1/11", "1/12");
        assert!(is_operational(&[], &operation, &setup.args).is_err());
        Ok(())
    }

    // --force-dir: error if src is a symlink to a directory
    #[test]
    #[cfg(target_family = "unix")]
    fn force_dir_src_is_symlink_to_directory_is_error() -> Result<()> {
        let mut setup = Setup::init("force_dir_src_is_symlink_to_directory_is_error")?;
        setup.args.force_dir = true;
        // create a symlink pointing to 1/11 (a directory)
        let link = setup.sandbox.join("link_to_src_dir");
        std::os::unix::fs::symlink(setup.sandbox.join("1/11"), &link)?;
        assert!(link.is_symlink() && link.is_dir());
        let operation = Operation {
            kind: OperationKind::Move,
            src: Source {
                text: link.to_string_lossy().to_string(),
                path: link.clone(),
                abs: link.normalize().unwrap().into_path_buf(),
                meta: link.symlink_metadata().unwrap(),
            },
            dst: setup.destination_from("1/12"),
        };
        assert!(is_operational(&[], &operation, &setup.args).is_err());
        Ok(())
    }

    // --force-dir: overwrites a symlink to a directory (symlink is removed, not the target)
    #[test]
    #[cfg(target_family = "unix")]
    fn force_dir_dst_is_symlink_to_directory_is_replaced() -> Result<()> {
        let mut setup = Setup::init("force_dir_dst_is_symlink_to_directory_is_replaced")?;
        setup.args.force_dir = true;
        // create a symlink pointing to 1/12 (a directory)
        let link = setup.sandbox.join("link_to_dst_dir");
        std::os::unix::fs::symlink(setup.sandbox.join("1/12"), &link)?;
        assert!(link.is_symlink() && link.is_dir());
        let operation = Operation {
            kind: OperationKind::Move,
            src: setup.source_from("1/11"),
            dst: Destination {
                text: link.to_string_lossy().to_string(),
                path: link.clone(),
            },
        };
        is_operational(&[], &operation, &setup.args)?;
        execute_move_or_copy(&operation, &setup.args)?;
        // link is replaced by the moved directory; original target (1/12) is untouched
        assert!(link.is_dir() && !link.is_symlink());
        assert!(setup.sandbox.join("1/12").is_dir());
        assert!(!setup.sandbox.join("1/11").exists());
        Ok(())
    }
}
