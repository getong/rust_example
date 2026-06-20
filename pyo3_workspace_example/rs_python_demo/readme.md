# 在 Rust 中使用 Python 的模块

本示例使用 `uv` 管理项目内 Python 虚拟环境，通过 `python_src/pyproject.toml` 的 `requires-python` 指定 Python 版本，并通过 `python_src/pyproject.toml` 和 `python_src/uv.lock` 管理 Python 包依赖：

```shell
uv venv --python 3.14 --managed-python python_src/.venv
uv sync --project python_src --python python_src/.venv/bin/python --managed-python --locked
cargo run
```

也可以直接运行脚本：

```shell
./scripts/run.sh
```

如果当前 shell 激活了其他虚拟环境，`uv sync --project python_src --python python_src/.venv/bin/python` 会明确把依赖装进项目内 `python_src/.venv`。程序启动时会忽略外部 `VIRTUAL_ENV`，并根据 `python_src/.venv/pyvenv.cfg` 设置嵌入式 Python 所需的 `PYTHONHOME` / `PYTHONPATH`，避免初始化时找不到标准库模块，例如 `encodings`；初始化完成后会把项目 `python_src/.venv` 的 `site-packages` 加入 `sys.path`。

在 Linux 上，脚本还会从 `python_src/.venv/bin/python` 的 `sysconfig` 读取 `LIBDIR`，并把它加入 `LD_LIBRARY_PATH`，让运行时动态链接器能找到 uv 管理的 `libpython3.x.so.1.0`。

`python_src/pyproject.toml` 中的 `requires-python` 是解释器版本要求；脚本会读取它并用 `uv venv --python ...` 创建 `python_src/.venv`。Python 包依赖写在 `python_src/pyproject.toml` 的 `dependencies` 中，`python_src/uv.lock` 负责锁定解析后的具体版本。

要将Python嵌入到Rust二进制文件中，需要确保Python安装包含一个共享库。
假设我们用的ubuntu操作系统，先需要安装好python3环境和pip3，安装方式如下：

```shell
sudo apt install python3-dev python3-pip
python3 -m pip install --upgrade pip
```

当安装好python3后，就可以通过 `cargo new rs-python-demo` 命令创建一个rust二进制应用，并在Cargo.toml中添加如下依赖：

```toml
[dependencies]
pyo3 = { version = "0.29.0", features = ["auto-initialize"] }
```

接着在main.rs中添加如下代码：

```rust
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;

fn main() -> PyResult<()> {
    Python::attach(|py| {
        let sys = py.import("sys")?; // 导入python sys包
        let version: String = sys.getattr("version")?.extract()?; // 调用sys.version命令获取python版本

        let locals = [("os", py.import("os")?)].into_py_dict(py)?; // 导入os模块
        let code = c"os.getenv('USER') or os.getenv('USERNAME') or 'Unknown'";
        let user: String = py.eval(code, None, Some(&locals))?.extract()?;

        println!("Hello {}, Python version {}", user, version);
        Ok(())
    })
}
```

当运行cargo run就会看到对应的提示，如下图所示：
![](../rs-python-run.jpg)
