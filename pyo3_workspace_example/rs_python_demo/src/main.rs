// 引入pyo3包
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> PyResult<()> {
    configure_python_environment();

    Python::attach(|py| {
        add_project_site_packages(py)?;

        let sys = py.import("sys")?; // 导入python sys包
        let version: String = sys.getattr("version")?.extract()?; // 调用sys.version命令获取python版本

        let locals = [("os", py.import("os")?)].into_py_dict(py)?; // 导入os模块
        let code = c"os.getenv('USER') or os.getenv('USERNAME') or 'Unknown'";
        let user: String = py.eval(code, None, Some(&locals))?.extract()?;
        let packaging_locals = [("version", py.import("packaging.version")?)].into_py_dict(py)?;
        let parsed_version: String = py
            .eval(
                c"str(version.Version('1.2.3'))",
                None,
                Some(&packaging_locals),
            )?
            .extract()?;

        println!(
            "Hello {}, Python version {}, packaging parsed {}",
            user, version, parsed_version
        );
        Ok(())
    })
}

fn configure_python_environment() {
    remove_python_env_var("VIRTUAL_ENV");

    if let Some(python_home) = python_home_from_project_venv() {
        set_python_env_var("PYTHONHOME", python_home);
    } else {
        remove_python_env_var("PYTHONHOME");
    }

    if let Some(python_path) = stdlib_python_path_from_project_venv() {
        set_python_env_var("PYTHONPATH", python_path);
    } else {
        remove_python_env_var("PYTHONPATH");
    }
}

fn set_python_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
    // SAFETY: this binary calls `configure_python_environment` from `main`
    // before PyO3 initializes Python and before this program starts any
    // threads, so there is no concurrent environment access from this process.
    unsafe {
        env::set_var(key, value);
    }
}

fn remove_python_env_var(key: &str) {
    // SAFETY: this binary calls `configure_python_environment` from `main`
    // before PyO3 initializes Python and before this program starts any
    // threads, so there is no concurrent environment access from this process.
    unsafe {
        env::remove_var(key);
    }
}

fn add_project_site_packages(py: Python<'_>) -> PyResult<()> {
    let project_venv = project_venv();
    if !project_venv.join("pyvenv.cfg").is_file() {
        return Ok(());
    }

    let sys = py.import("sys")?;
    let version_info = sys.getattr("version_info")?;
    let major: u8 = version_info.get_item(0)?.extract()?;
    let minor: u8 = version_info.get_item(1)?.extract()?;
    let site_packages = site_packages_path(project_venv, major, minor);

    if site_packages.is_dir() {
        let site_packages = site_packages.to_string_lossy().into_owned();
        let sys_path = sys.getattr("path")?;
        let is_present: bool = sys_path
            .call_method1("__contains__", (site_packages.as_str(),))?
            .extract()?;

        if !is_present {
            sys_path.call_method1("insert", (0, site_packages))?;
        }
    }

    Ok(())
}

fn project_venv() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".venv")
}

fn python_home_from_project_venv() -> Option<PathBuf> {
    let pyvenv_cfg = fs::read_to_string(project_venv().join("pyvenv.cfg")).ok()?;
    let python_home = pyvenv_cfg
        .lines()
        .find_map(|line| line.strip_prefix("home = "))?;

    base_python_prefix(PathBuf::from(python_home))
}

fn stdlib_python_path_from_project_venv() -> Option<String> {
    let pyvenv_cfg = fs::read_to_string(project_venv().join("pyvenv.cfg")).ok()?;
    let python_home = pyvenv_cfg
        .lines()
        .find_map(|line| line.strip_prefix("home = "))?;
    let python_version = pyvenv_cfg
        .lines()
        .find_map(|line| line.strip_prefix("version_info = "))
        .and_then(major_minor_version)?;
    let stdlib =
        base_python_prefix(PathBuf::from(python_home))?.join(format!("lib/python{python_version}"));
    let lib_dynload = stdlib.join("lib-dynload");
    let mut paths = vec![stdlib];

    if lib_dynload.is_dir() {
        paths.push(lib_dynload);
    }

    env::join_paths(paths)
        .ok()
        .map(|python_path| python_path.to_string_lossy().into_owned())
}

fn base_python_prefix(python_home: PathBuf) -> Option<PathBuf> {
    if python_home
        .file_name()
        .is_some_and(|name| name == "bin" || name == "Scripts")
    {
        python_home.parent().map(PathBuf::from)
    } else {
        Some(python_home)
    }
}

fn major_minor_version(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    Some(format!("{major}.{minor}"))
}

#[cfg(not(windows))]
fn site_packages_path(project_venv: PathBuf, major: u8, minor: u8) -> PathBuf {
    project_venv
        .join("lib")
        .join(format!("python{major}.{minor}"))
        .join("site-packages")
}

#[cfg(windows)]
fn site_packages_path(project_venv: PathBuf, _major: u8, _minor: u8) -> PathBuf {
    project_venv.join("Lib").join("site-packages")
}
