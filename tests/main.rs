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
#[serial]
fn rel_rename_file() -> Result<()> {
    let setup = &Setup::init("rename_file")?;
    let operation = &setup.operation_from("1/11/11.txt", "1/11/renamed-11.txt");
    execute_move_or_copy(operation, &setup.args)?;
    assert!(operation.dst.path.is_file());
    assert!(!operation.src.path.is_file());
    Ok(())
}

#[test]
#[serial]
fn rel_rename_dir() -> Result<()> {
    let setup = &Setup::init("rename_dir")?;
    let operation = &setup.operation_from("1/11", "1/renamed-11");
    execute_move_or_copy(operation, &setup.args)?;
    assert!(operation.dst.path.is_dir());
    assert!(!operation.src.path.is_dir());
    Ok(())
}

#[test]
#[serial]
fn rel_rename_dir_with_sub_dirs() -> Result<()> {
    let setup = &Setup::init("rename_dir_with_sub_dirs")?;
    let operation = &setup.operation_from("1", "renamed-1");
    execute_move_or_copy(operation, &setup.args)?;
    assert!(operation.dst.path.is_dir());
    assert!(!operation.src.path.is_dir());
    Ok(())
}

#[test]
#[serial]
fn rel_move_and_rename_file() -> Result<()> {
    let setup = &Setup::init("move_and_rename_file")?;
    let operation = &setup.operation_from("2/21/211/211.txt", "1/renamed-211.txt");
    execute_move_or_copy(operation, &setup.args)?;
    assert!(operation.dst.path.is_file());
    assert!(!operation.src.path.is_file());
    Ok(())
}

#[test]
#[serial]
fn rel_move_and_rename_directory() -> Result<()> {
    let setup = &Setup::init("move_and_rename_directory")?;
    let operation = &setup.operation_from("2/22", "1/3");
    execute_move_or_copy(operation, &setup.args)?;
    assert!(operation.dst.path.is_dir());
    assert!(!operation.src.path.is_dir());
    Ok(())
}

#[test]
#[serial]
fn rel_force_move_overwrites_existing_file() -> Result<()> {
    let mut setup = Setup::init("force_move_overwrites_existing_file")?;
    setup.args.force = true;
    // src: 1/11/11.txt, dst: 1/12/12.txt (existing file)
    let operation = setup.operation_from("1/11/11.txt", "1/12/12.txt");
    // is_operational should succeed
    is_operational(&[], &operation, &setup.args)?;
    // should overwrite on move
    execute_move_or_copy(&operation, &setup.args)?;
    assert!(operation.dst.path.is_file());
    assert!(!operation.src.path.exists());
    Ok(())
}

#[test]
#[serial]
fn rel_force_copy_overwrites_existing_file() -> Result<()> {
    let mut setup = Setup::init("force_copy_overwrites_existing_file")?;
    setup.args.force = true;
    setup.args.copy = true;
    // src: 1/11/11.txt, dst: 1/12/12.txt (existing file)
    let base = setup.operation_from("1/11/11.txt", "1/12/12.txt");
    let operation = Operation {
        kind: OperationKind::Copy,
        src: base.src,
        dst: base.dst,
    };
    is_operational(&[], &operation, &setup.args)?;
    execute_move_or_copy(&operation, &setup.args)?;
    // both src and dst should exist (src is preserved on copy)
    assert!(operation.dst.path.is_file());
    assert!(operation.src.path.exists());
    Ok(())
}

#[test]
#[serial]
fn rel_without_force_existing_dst_is_error() -> Result<()> {
    let setup = Setup::init("without_force_existing_dst_is_error")?;
    // setup.args.force = false (default)
    let operation = setup.operation_from("1/11/11.txt", "1/12/12.txt");
    assert!(is_operational(&[], &operation, &setup.args).is_err());
    Ok(())
}

#[test]
#[serial]
fn rel_force_src_is_directory_is_error() -> Result<()> {
    let mut setup = Setup::init("force_src_is_directory_is_error")?;
    setup.args.force = true;
    // src: 1/11 (directory), dst: 1/12/12.txt (existing file)
    let operation = setup.operation_from("1/11", "1/12/12.txt");
    assert!(is_operational(&[], &operation, &setup.args).is_err());
    Ok(())
}

#[test]
#[serial]
fn rel_force_dst_is_directory_is_error() -> Result<()> {
    let mut setup = Setup::init("force_dst_is_directory_is_error")?;
    setup.args.force = true;
    // src: 1/11/11.txt (file), dst: 1/12 (existing directory)
    let operation = setup.operation_from("1/11/11.txt", "1/12");
    assert!(is_operational(&[], &operation, &setup.args).is_err());
    Ok(())
}

#[test]
#[serial]
#[cfg(target_family = "unix")]
fn rel_force_dst_is_symlink_to_directory_is_error() -> Result<()> {
    let mut setup = Setup::init("force_dst_is_symlink_to_directory_is_error")?;
    setup.args.force = true;
    // create a symlink pointing to 1/12 (a directory)
    let link = PathBuf::from("link_to_dir");
    std::os::unix::fs::symlink("1/12", &link)?;
    assert!(link.is_symlink() && link.is_dir());
    // src: 1/11/11.txt, dst: link_to_dir (symlink to a directory)
    let operation = Operation {
        kind: OperationKind::Move,
        src: setup.source_from("1/11/11.txt"),
        dst: setup.destination_from("link_to_dir"),
    };
    assert!(is_operational(&[], &operation, &setup.args).is_err());
    Ok(())
}

#[test]
#[serial]
fn rel_force_dir_move_overwrites_existing_directory() -> Result<()> {
    let mut setup = Setup::init("force_dir_move_overwrites_existing_directory")?;
    setup.args.force_dir = true;
    // src: 1/11 (directory with 11.txt), dst: 1/12 (existing directory with 12.txt)
    let operation = setup.operation_from("1/11", "1/12");
    is_operational(&[], &operation, &setup.args)?;
    execute_move_or_copy(&operation, &setup.args)?;
    // dst now contains src's contents (11.txt), 12.txt is gone
    assert!(PathBuf::from("1/12").is_dir());
    assert!(PathBuf::from("1/12/11.txt").is_file());
    assert!(!PathBuf::from("1/12/12.txt").exists());
    assert!(!PathBuf::from("1/11").exists());
    Ok(())
}

#[test]
#[serial]
fn rel_force_dir_copy_merges_into_existing_directory() -> Result<()> {
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
    assert!(PathBuf::from("1/12/12.txt").is_file());
    assert!(PathBuf::from("1/12/11.txt").is_file());
    // src is preserved on copy
    assert!(PathBuf::from("1/11").is_dir());
    Ok(())
}

#[test]
#[serial]
fn rel_force_dir_move_overwrites_existing_file() -> Result<()> {
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

#[test]
#[serial]
fn rel_without_force_dir_existing_dst_directory_is_error() -> Result<()> {
    let setup = Setup::init("without_force_dir_existing_dst_directory_is_error")?;
    // src: 1/11 (directory), dst: 1/12 (existing directory)
    let operation = setup.operation_from("1/11", "1/12");
    assert!(is_operational(&[], &operation, &setup.args).is_err());
    Ok(())
}

#[test]
#[serial]
#[cfg(target_family = "unix")]
fn rel_force_dir_src_is_symlink_to_directory_is_error() -> Result<()> {
    let mut setup = Setup::init("force_dir_src_is_symlink_to_directory_is_error")?;
    setup.args.force_dir = true;
    // create a symlink pointing to 1/11 (a directory)
    let link = PathBuf::from("link_to_src_dir");
    std::os::unix::fs::symlink("1/11", &link)?;
    assert!(link.is_symlink() && link.is_dir());
    let operation = Operation {
        kind: OperationKind::Move,
        src: setup.source_from("link_to_src_dir"),
        dst: setup.destination_from("1/12"),
    };
    assert!(is_operational(&[], &operation, &setup.args).is_err());
    Ok(())
}

#[test]
#[serial]
#[cfg(target_family = "unix")]
fn rel_force_dir_dst_is_symlink_to_directory_is_replaced() -> Result<()> {
    let mut setup = Setup::init("force_dir_dst_is_symlink_to_directory_is_replaced")?;
    setup.args.force_dir = true;
    // create a symlink pointing to 1/12 (a directory)
    let link = PathBuf::from("link_to_dst_dir");
    std::os::unix::fs::symlink("1/12", &link)?;
    assert!(link.is_symlink() && link.is_dir());
    let operation = Operation {
        kind: OperationKind::Move,
        src: setup.source_from("1/11"),
        dst: setup.destination_from("link_to_dst_dir"),
    };
    is_operational(&[], &operation, &setup.args)?;
    execute_move_or_copy(&operation, &setup.args)?;
    // link is replaced by the moved directory; original target (1/12) is untouched
    assert!(PathBuf::from("link_to_dst_dir").is_dir() && !PathBuf::from("link_to_dst_dir").is_symlink());
    assert!(PathBuf::from("1/12").is_dir());
    assert!(!PathBuf::from("1/11").exists());
    Ok(())
}
