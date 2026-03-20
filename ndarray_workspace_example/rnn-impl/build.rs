use std::{env, path::PathBuf, process::Command};

fn main() {
  println!("cargo:rerun-if-env-changed=LIBTORCH");
  println!("cargo:rerun-if-env-changed=LIBTORCH_USE_PYTORCH");
  println!("cargo:rerun-if-env-changed=PYTHON");
  println!("cargo:rerun-if-env-changed=VIRTUAL_ENV");

  if let Some(libtorch_lib_dir) = find_libtorch_lib_dir() {
    add_rpath(&libtorch_lib_dir);
  } else {
    println!(
      "cargo:warning=Could not infer libtorch runtime library path; set \
       DYLD_LIBRARY_PATH/LD_LIBRARY_PATH if execution fails."
    );
  }
}

fn find_libtorch_lib_dir() -> Option<PathBuf> {
  if let Ok(libtorch_root) = env::var("LIBTORCH") {
    let root = PathBuf::from(libtorch_root);
    let lib_dir = root.join("lib");
    if lib_dir.is_dir() {
      return Some(lib_dir);
    }
    if root.is_dir() {
      return Some(root);
    }
  }

  if env::var("LIBTORCH_USE_PYTORCH").ok().as_deref() != Some("1") {
    return None;
  }

  torch_lib_dir_from_python()
}

fn torch_lib_dir_from_python() -> Option<PathBuf> {
  let mut candidates = Vec::new();
  if let Ok(py) = env::var("PYTHON") {
    candidates.push(py);
  }
  candidates.push("python".to_string());
  candidates.push("python3".to_string());

  let probe = "import os, torch; print(os.path.join(os.path.dirname(torch.__file__), 'lib'))";
  for python in candidates {
    let out = Command::new(&python).args(["-c", probe]).output();
    let Ok(out) = out else {
      continue;
    };
    if !out.status.success() {
      continue;
    }
    let lib = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if lib.is_empty() {
      continue;
    }
    let path = PathBuf::from(lib);
    if path.is_dir() {
      return Some(path);
    }
  }
  None
}

fn add_rpath(libtorch_lib_dir: &PathBuf) {
  let p = libtorch_lib_dir.to_string_lossy();
  if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", p);
  }
}
