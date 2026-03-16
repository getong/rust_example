use pyo3::{prelude::*, types::IntoPyDict};

fn main() -> PyResult<()> {
  Python::attach(|py| {
    let sys = py.import("sys")?;
    let version: String = sys.getattr("version")?.extract()?;

    let locals = [("os", py.import("os")?)].into_py_dict(py)?;
    let code = c"os.getenv('USER') or os.getenv('USERNAME') or 'Unknown'";
    let user: String = py.eval(code, None, Some(&locals))?.extract()?;

    println!("Hello {}, I'm Python {}", user, version);
    Ok(())
  })
}
