//! This test performs almost the same test as the lib.rs test except for the following:
//! - Input paths are relative path from sandbox.
//! - Change current directory to temporary (sandbox) directory for each test case.
//! - Tests are executed sequencially to get consistent results.

use moove::*;

use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::*;
use normpath::PathExt;
use serial_test::serial;

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
        let sandbox = &std::env::temp_dir().join("moove").join("tests").join(key);
        std::fs::create_dir_all(sandbox)?;
        std::env::set_current_dir(&sandbox)?;
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
        let path = PathBuf::from(if cfg!(target_family = "windows") {
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
        let path = PathBuf::from(if cfg!(target_family = "windows") {
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
        if let Err(_) = std::env::set_current_dir("..") {
            return println!(
                "Failed to change current directory to the parent {}",
                "..".yellow().underline()
            );
        }
        println!(
            "{} {}",
            "Removing".dimmed(),
            self.sandbox.to_string_lossy().dimmed()
        );
        std::fs::remove_dir_all(&self.sandbox).unwrap();
    }
}

#[test]
#[serial]
fn rel_list_sources_normally() -> Result<()> {
    let mut setup = Setup::init("list_sources_normally")?;
    setup.args.paths.push("1".to_owned());
    let sources = sources_from(&setup.args)?;
    assert_eq!(sources[0].path, PathBuf::from("1/1.txt"));
    assert_eq!(sources[1].path, PathBuf::from("1/11"));
    assert_eq!(sources[2].path, PathBuf::from("1/12"));
    Ok(())
}

#[test]
#[serial]
fn rel_operate_normally() -> Result<()> {
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
#[serial]
fn rel_rename_file() -> Result<()> {
    let setup = &Setup::init("rename_file")?;
    let operation = &setup.operation_from("1/11/11.txt", "1/11/renamed-11.txt");
    execute_move(operation, &setup.args)?;
    assert!(operation.dst.path.is_file());
    assert!(!operation.src.path.is_file());
    Ok(())
}

#[test]
#[serial]
fn rel_rename_dir() -> Result<()> {
    let setup = &Setup::init("rename_dir")?;
    let operation = &setup.operation_from("1/11", "1/renamed-11");
    execute_move(operation, &setup.args)?;
    assert!(operation.dst.path.is_dir());
    assert!(!operation.src.path.is_dir());
    Ok(())
}

#[test]
#[serial]
fn rel_rename_dir_with_sub_dirs() -> Result<()> {
    let setup = &Setup::init("rename_dir_with_sub_dirs")?;
    let operation = &setup.operation_from("1", "renamed-1");
    execute_move(operation, &setup.args)?;
    assert!(operation.dst.path.is_dir());
    assert!(!operation.src.path.is_dir());
    Ok(())
}

#[test]
#[serial]
fn rel_move_and_rename_file() -> Result<()> {
    let setup = &Setup::init("move_and_rename_file")?;
    let operation = &setup.operation_from("2/21/211/211.txt", "1/renamed-211.txt");
    execute_move(operation, &setup.args)?;
    assert!(operation.dst.path.is_file());
    assert!(!operation.src.path.is_file());
    Ok(())
}

#[test]
#[serial]
fn rel_move_and_rename_directory() -> Result<()> {
    let setup = &Setup::init("move_and_rename_directory")?;
    let operation = &setup.operation_from("2/22", "1/3");
    execute_move(operation, &setup.args)?;
    assert!(operation.dst.path.is_dir());
    assert!(!operation.src.path.is_dir());
    Ok(())
}
