use std::{
  fs,
  path::Path,
  process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_walkdir_mv_example"))
    .current_dir(root)
    .env_remove("WALKDIR_DELETE_TARGET_DIR")
    .args(args)
    .output()
    .unwrap()
}

#[test]
fn explicit_target_preserves_data_and_unrelated_directories() {
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  fs::create_dir_all(root.join("target/group/wrapper/empty")).unwrap();
  fs::create_dir_all(root.join("target/unrelated/empty")).unwrap();
  fs::create_dir_all(root.join("outside/group/wrapper")).unwrap();
  fs::create_dir_all(root.join("target/.hidden/wrapper")).unwrap();
  for (path, content) in [
    ("target/.DS_Store", "root metadata"),
    ("target/group/wrapper/.DS_Store", "child metadata"),
    ("target/group/wrapper/data.txt", "important data"),
    ("target/.hidden/wrapper/secret", "secret"),
    ("outside/group/wrapper/data.txt", "outside"),
  ] {
    fs::write(root.join(path), content).unwrap();
  }
  let output = run(root, &["--dir", "target"]);
  assert!(output.status.success(), "{output:?}");
  assert_eq!(
    fs::read_to_string(root.join("target/group/data.txt")).unwrap(),
    "important data"
  );
  assert_eq!(
    fs::read_to_string(root.join("target/group/.DS_Store")).unwrap(),
    "child metadata"
  );
  assert_eq!(
    fs::read_to_string(root.join("target/.DS_Store")).unwrap(),
    "root metadata"
  );
  assert!(root.join("target/group/empty").is_dir());
  assert!(!root.join("target/group/wrapper").exists());
  assert!(root.join("target/unrelated/empty").is_dir());
  assert!(root.join("target/.hidden/wrapper/secret").is_file());
  assert!(root.join("outside/group/wrapper/data.txt").is_file());
}

#[test]
fn conflicts_preserve_both_files_and_source_directory() {
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  fs::create_dir_all(root.join("group/wrapper")).unwrap();
  fs::write(root.join("group/.keep"), "destination").unwrap();
  fs::write(root.join("group/wrapper/.keep"), "source").unwrap();
  fs::write(root.join("group/wrapper/other"), "other").unwrap();
  assert!(run(root, &["--dir=."]).status.success());
  assert_eq!(
    fs::read_to_string(root.join("group/.keep")).unwrap(),
    "destination"
  );
  assert_eq!(
    fs::read_to_string(root.join("group/wrapper/.keep")).unwrap(),
    "source"
  );
  assert_eq!(
    fs::read_to_string(root.join("group/other")).unwrap(),
    "other"
  );
}

#[cfg(unix)]
#[test]
fn dangling_destination_symlink_is_never_overwritten() {
  use std::os::unix::fs::symlink;
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  fs::create_dir_all(root.join("group/wrapper")).unwrap();
  symlink("missing", root.join("group/.link")).unwrap();
  fs::write(root.join("group/wrapper/.link"), "source").unwrap();
  assert!(run(root, &["--dir", "."]).status.success());
  assert_eq!(
    fs::read_link(root.join("group/.link")).unwrap(),
    Path::new("missing")
  );
  assert_eq!(
    fs::read_to_string(root.join("group/wrapper/.link")).unwrap(),
    "source"
  );
}

#[cfg(unix)]
#[test]
fn moves_preserve_inode_and_do_not_follow_directory_symlinks() {
  use std::os::unix::fs::{MetadataExt, symlink};
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  fs::create_dir_all(root.join("target/group/wrapper")).unwrap();
  fs::create_dir_all(root.join("outside/group/wrapper")).unwrap();
  fs::write(root.join("outside/group/wrapper/file"), "outside").unwrap();
  symlink(root.join("outside"), root.join("target/link")).unwrap();
  let src = root.join("target/group/wrapper/file");
  fs::write(&src, "data").unwrap();
  let inode = fs::metadata(&src).unwrap().ino();
  symlink("missing", root.join("target/group/wrapper/link")).unwrap();
  assert!(run(root, &["--dir=target"]).status.success());
  assert_eq!(
    fs::metadata(root.join("target/group/file")).unwrap().ino(),
    inode
  );
  assert_eq!(
    fs::read_link(root.join("target/group/link")).unwrap(),
    Path::new("missing")
  );
  assert!(root.join("outside/group/wrapper/file").is_file());
}

#[test]
fn invalid_arguments_never_modify_the_working_directory() {
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  fs::create_dir_all(root.join("group/wrapper")).unwrap();
  fs::write(root.join("group/wrapper/data"), "keep").unwrap();
  fs::write(root.join(".DS_Store"), "keep").unwrap();
  for args in [
    vec![],
    vec!["--di", "."],
    vec!["--dir"],
    vec!["--dir="],
    vec!["--dir", "--typo"],
    vec!["--dir", ".", "--typo"],
    vec!["--dir=.", "--dir=missing"],
    vec!["--dir", "missing"],
    vec!["--dir", ".DS_Store"],
  ] {
    assert!(!run(root, &args).status.success(), "{args:?}");
    assert!(root.join("group/wrapper/data").is_file());
    assert!(root.join(".DS_Store").is_file());
  }
}

#[test]
fn environment_target_still_works_and_explicit_dir_takes_precedence() {
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  fs::create_dir_all(root.join("target/group/wrapper")).unwrap();
  fs::write(root.join("target/group/wrapper/data"), "keep").unwrap();
  let output = Command::new(env!("CARGO_BIN_EXE_walkdir_mv_example"))
    .current_dir(root)
    .env("WALKDIR_DELETE_TARGET_DIR", "missing")
    .args(["--dir", "target"])
    .output()
    .unwrap();
  assert!(output.status.success(), "{output:?}");
  assert!(root.join("target/group/data").is_file());
  let output = Command::new(env!("CARGO_BIN_EXE_walkdir_mv_example"))
    .current_dir(root)
    .env("WALKDIR_DELETE_TARGET_DIR", "target")
    .output()
    .unwrap();
  assert!(output.status.success(), "{output:?}");
}
