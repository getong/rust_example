# 在 Rust 中使用 Python 的模块

本示例使用 `uv` 管理项目内 Python 虚拟环境，通过 `pyproject.toml` 的 `requires-python = "==3.13.*"` 锁定 Python 3.13 系列，并通过 `requirement.txt` 锁定 Python 包依赖：

```shell
uv venv --python 3.13 --managed-python .venv
uv pip install --python .venv/bin/python -r requirement.txt
cargo run
```

也可以直接运行脚本：

```shell
./scripts/run.sh
```

如果当前 shell 激活了其他虚拟环境，`uv pip install --python .venv/bin/python` 会明确把依赖装进项目内 `.venv`。程序启动时会忽略外部 `VIRTUAL_ENV`，并根据 `.venv/pyvenv.cfg` 设置嵌入式 Python 所需的 `PYTHONHOME` / `PYTHONPATH`，避免初始化时找不到标准库模块，例如 `encodings`；初始化完成后会把项目 `.venv` 的 `site-packages` 加入 `sys.path`。

在 Linux 上，脚本还会从 `.venv/bin/python` 的 `sysconfig` 读取 `LIBDIR`，并把它加入 `LD_LIBRARY_PATH`，让运行时动态链接器能找到 uv 管理的 `libpython3.x.so.1.0`。

`pyproject.toml` 中的 `requires-python = "==3.13.*"` 是解释器版本锁；脚本会读取它并用 `uv venv --python 3.13` 创建 `.venv`。如果需要锁定到具体 patch 版本，也可以写成 `requires-python = "==3.13.14"`。`requirement.txt` 中的 `packaging==25.0` 是 Python 包版本锁。

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
