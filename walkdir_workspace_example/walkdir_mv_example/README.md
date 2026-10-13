# walkdir_mv_example

```sh
cargo run -p walkdir_mv_example -- --dir '/path/to/target'
# Also accepts --dir=/path/to/target
```

只检查目标目录下的直接、非隐藏子目录。如果某个子目录只有一个可见目录、没有可见文件，就将该目录里的条目向上移动一层。

- 文件与目录直接重命名，不复制；macOS/Linux/Android 使用原子的不覆盖移动。
- 目标已存在时跳过，保留两边的数据，包括失效的符号链接。
- 保留 `.DS_Store`、隐藏文件和原有空目录。只尝试删除本次成功移出条目后留下的空包装目录；仍有内容就保留。
- 不遍历子目录符号链接，不处理目标目录下的隐藏目录。
- 跨文件系统或不支持安全移动的平台/文件系统会报错，不回退到覆盖或复制删除。
- 缺少、重复或未知参数会报错，不再默认操作当前目录。要操作当前目录，请显式使用 `--dir .`。未指定 `--dir` 时仍兼容 `WALKDIR_DELETE_TARGET_DIR` 环境变量。

例如：`target/group/wrapper/file` 会移动到 `target/group/file`。发生冲突时，未移动的条目仍在 `wrapper` 中。多文件移动不是事务；发生错误时，之前已成功移动的条目会保留在新位置。

验证：

```sh
cargo test -p walkdir_mv_example
cargo clippy -p walkdir_mv_example --all-targets -- -D warnings
```
