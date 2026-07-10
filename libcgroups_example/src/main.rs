//! `libcgroups` 功能演示。
//!
//! cgroup 是 Linux 内核对“一组进程”进行资源治理的机制；`libcgroups`
//! 则把 cgroup v1、v2 和 systemd 后端封装成统一的 Rust API。

#[cfg(target_os = "linux")]
mod linux {
  use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::Duration,
  };

  use libcgroups::common::{
    CgroupConfig, CgroupManager, ControllerOpt, create_cgroup_manager_with_root,
    get_cgroup_setup_with_root,
  };
  use nix::unistd::Pid;
  use oci_spec::runtime::{
    LinuxCpuBuilder, LinuxMemoryBuilder, LinuxPidsBuilder, LinuxResources, LinuxResourcesBuilder,
  };

  type Result<T> = std::result::Result<T, Box<dyn Error>>;

  const DEFAULT_CGROUP_ROOT: &str = "/sys/fs/cgroup";
  const MEMORY_LIMIT_BYTES: i64 = 64 * 1024 * 1024;
  const CPU_PERIOD_MICROS: u64 = 100_000;
  const CPU_QUOTA_MICROS: i64 = 50_000;
  const PID_LIMIT: i64 = 16;

  pub fn main() -> Result<()> {
    print_purpose();

    let mut args = env::args().skip(1);
    let command = args.next();
    let root = args
      .next()
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from(DEFAULT_CGROUP_ROOT));

    let setup = get_cgroup_setup_with_root(&root)?;
    println!("\n检测结果：{} 使用 {setup} 模式。", root.display());

    match command.as_deref() {
      None | Some("inspect") => print_inspect_hint(),
      Some("demo") => run_demo(&root)?,
      Some(other) => {
        return Err(
          format!("未知命令 {other:?}；用法：cargo run -- [inspect | demo [CGROUP_ROOT]]").into(),
        );
      }
    }

    Ok(())
  }

  fn print_purpose() {
    println!(
      "libcgroups 的主要作用：\n1. 创建并选择 cgroup v1、v2 或 systemd 管理器；\n2. 把进程加入 \
       cgroup，并限制 CPU、内存、PID、I/O 等资源；\n3. 查询 CPU、内存、PID 和块设备 I/O \
       使用统计；\n4. 冻结/恢复一组进程，并在结束时删除 cgroup。"
    );
  }

  fn print_inspect_hint() {
    println!(
      "当前是只读的 inspect 模式，没有创建或修改 cgroup。\n要运行资源限制演示，请执行：cargo run \
       -- demo\n该操作需要 Linux cgroup 的写权限（通常是 root 或已委派的 cgroup 子树）。"
    );
  }

  fn run_demo(root: &Path) -> Result<()> {
    let cgroup_name = format!("libcgroups-example-{}", std::process::id());
    let config = CgroupConfig {
      cgroup_path: PathBuf::from(&cgroup_name),
      // 本例使用文件系统 API；生产环境也可让 systemd 管理 cgroup 生命周期。
      systemd_cgroup: false,
      container_name: cgroup_name.clone(),
    };
    let manager = create_cgroup_manager_with_root(Some(root), config)?;
    let resources = demo_resources()?;

    // 只把专门创建的子进程放入 cgroup。若把当前进程放进去，清理 cgroup 时
    // 可能会连示例程序本身一起终止。
    let mut child = Command::new("sleep").arg("30").spawn()?;
    let demo_result = apply_and_observe(&manager, &resources, &mut child);

    // 无论应用限制或读取统计是否成功，都先结束子进程，再删除 cgroup。
    let _ = child.kill();
    let _ = child.wait();
    let cleanup_result = manager.remove();

    demo_result?;
    cleanup_result?;
    println!("演示完成，cgroup {cgroup_name:?} 已删除。");
    Ok(())
  }

  fn demo_resources() -> Result<LinuxResources> {
    let memory = LinuxMemoryBuilder::default()
      .limit(MEMORY_LIMIT_BYTES)
      .build()?;
    let cpu = LinuxCpuBuilder::default()
      // 每 100 ms 最多运行 50 ms，约等于半个 CPU 核心的时间配额。
      .period(CPU_PERIOD_MICROS)
      .quota(CPU_QUOTA_MICROS)
      .build()?;
    let pids = LinuxPidsBuilder::default().limit(PID_LIMIT).build()?;

    Ok(
      LinuxResourcesBuilder::default()
        .memory(memory)
        .cpu(cpu)
        .pids(pids)
        .build()?,
    )
  }

  fn apply_and_observe(
    manager: &libcgroups::common::AnyCgroupManager,
    resources: &LinuxResources,
    child: &mut Child,
  ) -> Result<()> {
    let child_pid = i32::try_from(child.id())?;

    // add_task 会创建 cgroup（若尚不存在），并把指定 PID 写入 cgroup.procs。
    manager.add_task(Pid::from_raw(child_pid))?;

    // apply 把 OCI LinuxResources 转成各版本 cgroup 控制文件的设置。
    manager.apply(&ControllerOpt {
      resources,
      disable_oom_killer: false,
      oom_score_adj: None,
      freezer_state: None,
    })?;

    thread::sleep(Duration::from_millis(100));
    let pids = manager.get_all_pids()?;
    let stats = manager.stats()?;

    println!(
      "\n已把子进程 {child_pid} 加入 cgroup：\n- 内存上限：{} MiB\n- CPU 配额：{}/{} 微秒\n- PID \
       上限：{}\n- cgroup 内 PID：{:?}\n- 当前内存：{} 字节\n- 已用 CPU：{}\n- 当前 PID 数：{}",
      MEMORY_LIMIT_BYTES / 1024 / 1024,
      CPU_QUOTA_MICROS,
      CPU_PERIOD_MICROS,
      PID_LIMIT,
      pids,
      stats.memory.memory.usage,
      stats.cpu.usage.usage_total,
      stats.pids.current,
    );

    Ok(())
  }
}

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
  linux::main()
}

#[cfg(not(target_os = "linux"))]
fn main() {
  println!(
    "libcgroups 是 Linux cgroup 的 Rust 封装，用来限制、统计、冻结和清理一组进程。\n当前系统是 \
     {}，没有 Linux cgroup；请在 Linux 主机或 Linux 容器中运行本例。",
    std::env::consts::OS
  );
}
