use deno_os::sys_info;

#[tokio::main]
async fn main() {
  println!("sys hostname is {:?}", sys_info::hostname());
  println!("sys loadavg is {:?}", sys_info::loadavg());
  if let Some(mem_info) = sys_info::mem_info() {
    let mem_info_json = serde_json::json!({
      "total": mem_info.total,
      "free": mem_info.free,
      "available": mem_info.available,
      "buffers": mem_info.buffers,
      "cached": mem_info.cached,
      "swap_total": mem_info.swap_total,
      "swap_free": mem_info.swap_free,
    });
    println!("sys mem info is {:?}", mem_info_json);
  }

  println!("sys os_release is {:?}", sys_info::os_release());
  println!("sys os_uptime is {:?}", sys_info::os_uptime());
}
