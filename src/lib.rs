use std::fs::Metadata;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use fs_extra::dir::CopyOptions;
use normpath::PathExt;

#[derive(Debug, Parser, Default)]
#[command(version)]
pub struct CommandLine {
    /// Paths to move
    #[arg(default_value = ".", value_hint = clap::ValueHint::AnyPath)]
    pub paths: Vec<String>,
    /// Dry-run option
    #[arg(short = 'u', long)]
    pub dry_run: bool,
    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
    /// No output to stdout/strerr even if error
    #[arg(short, long)]
    pub quiet: bool,
    /// Handle as absolute paths
    #[arg(short, long)]
    pub absolute: bool,
    /// Directories themselves, not their contents
    #[arg(short, long)]
    pub directory: bool,
}

#[derive(Debug)]
struct Operation {
    kind: OperationKind,
    src: Source,
    dst: Destination,
}

#[derive(Debug)]
enum OperationKind {
    Move,
}

#[derive(Debug, Clone)]
struct Source {
    text: String,
    path: PathBuf,
    abs: PathBuf,
    meta: Metadata,
}

#[derive(Debug, Clone)]
struct Destination {
    text: String,
    path: PathBuf,
    normalized: PathBuf,
}

static SEPARATORS: &[char] = &['/', '\\'];

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

fn sources_from(args: &CommandLine) -> Result<Vec<Source>> {
    let mut sources: Vec<Source> = Vec::new();
    let paths = arg_paths(&args.paths)?;
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
                put_source(&mut children, &entry?.path(), args)?;
            }
            children.sort_unstable_by(|a, b| a.abs.cmp(&b.abs));
            sources.append(&mut children);
            if sources.is_empty() {
                anyhow::bail!(
                    "Directory is empty. {}\n\
                    Use --directory for the directory itself.",
                    path.to_string_lossy().yellow().underline()
                );
            }
        }
    }
    Ok(sources)
}

fn arg_paths(args: &[String]) -> Result<Vec<String>> {
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
        globbed.sort_unstable_by_key(|a| a.canonicalize().unwrap());
        paths.append(
            &mut globbed
                .iter()
                .map(|g| g.to_string_lossy().to_string())
                .collect(),
        );
    }
    Ok(paths)
}

fn put_source(sources: &mut Vec<Source>, path: &Path, args: &CommandLine) -> Result<()> {
    let normalized = normalize(path)?;
    if normalized.parent().is_err() {
        anyhow::bail!(
            "Source should not be the root directory. {}",
            path.to_string_lossy().yellow().underline()
        );
    }
    let new_path = if args.absolute {
        normalized.as_path()
    } else {
        path
    };
    let new_src = Source {
        text: new_path
            .to_str()
            .with_context(|| {
                format!(
                    "Failed to convert path to UTF-8. {}",
                    path.to_string_lossy().to_string().yellow().underline()
                )
            })?
            .trim_end_matches(SEPARATORS)
            .to_string(),
        path: new_path.to_path_buf(),
        abs: normalized.as_path().to_path_buf(),
        meta: new_path.symlink_metadata().with_context(|| {
            format!(
                "Failed to access {}",
                new_path.to_string_lossy().yellow().underline()
            )
        })?,
    };
    for src in sources.iter().map(|s| s.abs.as_path()) {
        if src == new_src.abs {
            anyhow::bail!(
                "Duplicated source. {}",
                new_src.abs.to_string_lossy().yellow().underline()
            );
        }
    }
    sources.push(new_src);
    Ok(())
}

#[cfg(target_family = "windows")]
fn normalize(path: &Path) -> Result<normpath::BasePathBuf> {
    path.normalize_virtually().with_context(|| {
        format!(
            "Failed to normalize path. {}",
            path.to_string_lossy().yellow().underline()
        )
    })
}

#[cfg(target_family = "unix")]
fn normalize(path: &Path) -> Result<normpath::BasePathBuf> {
    path.normalize().with_context(|| {
        format!(
            "Failed to normalize path. {}",
            path.to_string_lossy().yellow().underline()
        )
    })
}

fn operations_from(sources: &Vec<Source>, _args: &CommandLine) -> Result<Vec<Operation>> {
    let lines = edit::edit(
        sources
            .iter()
            .map(|src| {
                let mut line = src.text.to_owned();
                if src.path.is_dir() && !src.path.is_symlink() && !line.ends_with(SEPARATORS) {
                    line.push(std::path::MAIN_SEPARATOR);
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )?
    .split('\n')
    .filter_map(|line| {
        let line = line.trim().trim_end_matches(SEPARATORS);
        if line.is_empty() {
            None
        } else {
            Some(line.to_string())
        }
    })
    .collect::<Vec<_>>();
    if lines.len() != sources.len() {
        anyhow::bail!(
            "Number of lines {} does not match the original one {}",
            lines.len().to_string().yellow(),
            sources.len().to_string().yellow()
        );
    }
    let mut operations = Vec::new();
    for (src, line) in sources.iter().zip(lines.iter()) {
        if &src.text == line {
            continue;
        }
        let dst_path = PathBuf::from(&line);
        let new_operation = Operation {
            kind: OperationKind::Move,
            src: src.to_owned(),
            dst: Destination {
                text: line.to_string(),
                path: dst_path.to_owned(),
                normalized: normalize_lexically(&dst_path),
            },
        };
        is_operational(&operations, &new_operation)?;
        operations.push(new_operation);
    }
    Ok(operations)
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component);
                }
            }
            _ => {
                normalized.push(component);
            }
        }
    }
    normalized
}

fn is_operational(operations: &[Operation], new_operation: &Operation) -> Result<()> {
    let src = &new_operation.src;
    let dst = &new_operation.dst;
    if dst.text.ends_with(SEPARATORS) && (src.meta.is_file() || src.meta.is_symlink()) {
        anyhow::bail!(
            "Missing file name. {} for {}",
            dst.text.yellow().underline(),
            src.text.underline()
        )
    }
    if operations
        .iter()
        .any(|o| o.dst.normalized == dst.normalized)
    {
        anyhow::bail!("Duplicated destination. {}", dst.text.yellow().underline());
    }
    if operations
        .iter()
        .any(|o| o.dst.path.ancestors().any(|a| a == dst.path))
    {
        anyhow::bail!(
            "Destination should not be included in other destination. {}",
            dst.text.yellow().underline()
        );
    }
    if dst.path.exists() {
        anyhow::bail!("Destination exists. {}", dst.text.yellow().underline())
    }
    if dst.path.ancestors().any(|d| d == src.path) {
        anyhow::bail!(
            "Destination should not be included in source.\n\
             Source:      {}\n\
             Destination: {}",
            dst.text.yellow().underline(),
            src.text.yellow().underline()
        );
    }
    Ok(())
}

fn execute_operation(o: &Operation, args: &CommandLine) -> Result<()> {
    match o.kind {
        OperationKind::Move => {
            if !args.quiet && (args.verbose || args.dry_run) {
                println!(
                    "{} {}{}{}",
                    "Move".dimmed(),
                    o.src.text.dimmed().underline(),
                    " → ".dimmed(),
                    o.dst.text.dimmed().underline()
                );
            }
            if args.dry_run {
                return Ok(());
            }
            execute_move(o, args)?;
            if !args.quiet {
                println!(
                    "{} → {}",
                    o.src.text.green().underline(),
                    o.dst.text.green().underline()
                );
            }
        }
    };
    Ok(())
}

fn execute_move(operation: &Operation, args: &CommandLine) -> Result<()> {
    let Operation { src, dst, .. } = operation;
    //
    // Create parent directory if missing.
    //
    let current_dir = std::env::current_dir().context("Failed to get current directory.")?;
    let dst_parent = if dst.text.contains(SEPARATORS) {
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
    //
    // Move source if its parent need to be changed.
    //
    // NOTE Can be unwrapped safely, `src` always has the parent.
    if src.abs.parent().unwrap() != dst_parent {
        if !args.quiet && args.verbose {
            println!(
                "{} {} {}",
                "Moving".dimmed(),
                src.abs.to_string_lossy().dimmed().underline(),
                dst_parent.to_string_lossy().dimmed().underline()
            );
        }
        fs_extra::move_items(&[&src.path], dst_parent, &CopyOptions::default()).with_context(
            || {
                format!(
                    "Failed to move {} to {}",
                    src.text.yellow().underline(),
                    dst_parent.to_string_lossy().yellow().underline()
                )
            },
        )?;
    }
    //
    // Rename if its file name need to be changed.
    //
    // NOTE Can be unwrapped safely, `src` and `dst` always have basename.
    let src_basename = src.path.file_name().unwrap();
    let dst_basename = dst.path.file_name().unwrap();
    if src_basename != dst_basename {
        let rename_from = &dst_parent.join(src_basename);
        let rename_to = &dst_parent.join(dst_basename);
        if !args.quiet && args.verbose {
            println!(
                "{} {}{}{}",
                "Renaming".dimmed(),
                rename_from.to_string_lossy().dimmed().underline(),
                " → ".dimmed(),
                rename_to.to_string_lossy().dimmed().underline()
            );
        }
        // Destination is never over-written.
        // It was ensured when the operation was made.
        std::fs::rename(rename_from, rename_to).with_context(|| {
            format!(
                "Failed to rename {} to {}",
                rename_from.to_string_lossy().yellow().underline(),
                rename_to.to_string_lossy().yellow().underline()
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod lib {
    use std::path::PathBuf;

    use anyhow::Result;
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
                .map(|d| sandbox.join(d))
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
            let path = self.sandbox.join(s);
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
            let path = self.sandbox.join(s);
            Destination {
                text: path.to_string_lossy().to_string(),
                path: path.to_owned(),
                normalized: normalize_lexically(&path),
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
        Ok(())
    }

    #[test]
    fn operate_normally() -> Result<()> {
        let setup = &Setup::init("operate_normally")?;
        let mut operations = Vec::new();
        let new_operation = setup.operation_from("1/11/11.txt", "1/12/moved-11.txt");
        is_operational(&operations, &new_operation)?;
        operations.push(new_operation);
        let new_operation = setup.operation_from("1/12/12.txt", "1/11/moved-12.txt");
        is_operational(&operations, &new_operation)?;
        operations.push(new_operation);
        let new_operation = setup.operation_from("1/1.txt", "1/11/moved-1.txt");
        is_operational(&operations, &new_operation)?;
        operations.push(new_operation);
        let new_operation = setup.operation_from("2/21/211", "moved-211");
        is_operational(&operations, &new_operation)?;
        operations.push(new_operation);
        let new_operation = setup.operation_from("2/22", "moved-211/moved-22");
        is_operational(&operations, &new_operation)?;
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
            assert!(is_operational(&operations, &setup.operation_from(src, dst)).is_err());
        });
        Ok(())
    }

    #[test]
    fn rename_file() -> Result<()> {
        let setup = &Setup::init("rename_file")?;
        let operation = &setup.operation_from("1/11/11.txt", "1/11/renamed-11.txt");
        execute_move(operation, &setup.args)?;
        assert!(operation.dst.path.is_file());
        assert!(!operation.src.path.is_file());
        Ok(())
    }

    #[test]
    fn rename_dir() -> Result<()> {
        let setup = &Setup::init("rename_dir")?;
        let operation = &setup.operation_from("1/11", "1/renamed-11");
        execute_move(operation, &setup.args)?;
        assert!(operation.dst.path.is_dir());
        assert!(!operation.src.path.is_dir());
        Ok(())
    }

    #[test]
    fn rename_dir_with_sub_dirs() -> Result<()> {
        let setup = &Setup::init("rename_dir_with_sub_dirs")?;
        let operation = &setup.operation_from("1", "renamed-1");
        execute_move(operation, &setup.args)?;
        assert!(operation.dst.path.is_dir());
        assert!(!operation.src.path.is_dir());
        Ok(())
    }

    #[test]
    fn move_and_rename_file() -> Result<()> {
        let setup = &Setup::init("move_and_rename_file")?;
        let operation = &setup.operation_from("2/21/211/211.txt", "1/renamed-211.txt");
        execute_move(operation, &setup.args)?;
        assert!(operation.dst.path.is_file());
        assert!(!operation.src.path.is_file());
        Ok(())
    }

    #[test]
    fn move_and_rename_directory() -> Result<()> {
        let setup = &Setup::init("move_and_rename_directory")?;
        let operation = &setup.operation_from("2/22", "1/3");
        execute_move(operation, &setup.args)?;
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
}
