use deno_os::sys_info;

#[tokio::main]
async fn main() {
  println!("sys hostname is {:?}", sys_info::hostname());
  println!("sys loadavg is {:?}", sys_info::loadavg());
  if let Some(mem_info) = sys_info::mem_info() {
    if let Ok(mem_info_json) = serde_json::to_string(&mem_info) {
      println!("sys mem info is {:?}", mem_info_json);
    }
  }

  println!("sys os_release is {:?}", sys_info::os_release());
  println!("sys os_uptime is {:?}", sys_info::os_uptime());
}
