// ash 是 Vulkan API 的 Rust 绑定（轻量、接近原生 C API）。
//
// Vulkan 是什么、有什么用：
// - Vulkan 是 Khronos 制定的跨平台、低开销的现代图形 + 通用 GPU 计算 API， 是 OpenGL
//   的继任者。它把驱动隐式管理的东西（内存分配、同步、命令提交、
//   多线程录制）全部显式交给应用程序控制，从而获得更低的 CPU 开销和更可预测的性能。
// - 典型用途：游戏渲染引擎、GPU 计算（compute shader）、视频处理、跨平台图形中间层 （如 wgpu、Skia
//   的后端）。
//
// macOS 的特殊性：
// - macOS 原生只有 Metal，没有系统级 Vulkan 驱动。Vulkan 在 macOS 上通过 MoltenVK（把 Vulkan
//   调用转译为 Metal）来实现，属于"portability（可移植性）实现"， 并非完全符合 Vulkan 规范。
// - 因此在 macOS 上必须：
//   1. 创建 Instance 时启用 VK_KHR_portability_enumeration 扩展，并设置 ENUMERATE_PORTABILITY_KHR
//      标志，否则新版 loader 会直接忽略 MoltenVK 设备；
//   2. 创建逻辑设备时，如果物理设备暴露了 VK_KHR_portability_subset 扩展， 规范要求必须显式启用它。
// - 本机通过 Homebrew 安装：brew install molten-vk vulkan-loader

use std::error::Error;

use ash::{Entry, vk};

fn main() -> Result<(), Box<dyn Error>> {
  // ---------- 1. Entry：加载 Vulkan 动态库 ----------
  // Vulkan 没有全局链接符号，函数指针都要在运行时从 loader（libvulkan.dylib）取出。
  // Entry 持有的就是这批"全局级"函数（创建 Instance、查询版本等）。
  let entry = load_entry()?;

  let version = unsafe { entry.try_enumerate_instance_version()? }.unwrap_or(vk::API_VERSION_1_0);
  println!(
    "Vulkan instance 版本: {}.{}.{}",
    vk::api_version_major(version),
    vk::api_version_minor(version),
    vk::api_version_patch(version)
  );

  // ---------- 2. Instance：应用与 Vulkan 库的连接 ----------
  // Instance 是一切的入口：它初始化 loader、聚合系统里所有的驱动（ICD），
  // 之后才能枚举 GPU。ApplicationInfo 里的信息可供驱动做针对性优化。
  let app_info = vk::ApplicationInfo::default()
    .application_name(c"ash_example")
    .application_version(vk::make_api_version(0, 1, 0, 0))
    .engine_name(c"no_engine")
    .api_version(vk::API_VERSION_1_2);

  // macOS(MoltenVK) 必须启用 portability enumeration，才能把"非完全符合规范"的
  // MoltenVK 设备枚举出来；VK_KHR_get_physical_device_properties2 是
  // portability_subset 设备扩展的依赖项。
  let mut instance_extensions: Vec<*const std::os::raw::c_char> = Vec::new();
  let mut instance_flags = vk::InstanceCreateFlags::empty();
  if cfg!(target_os = "macos") {
    instance_extensions.push(ash::khr::portability_enumeration::NAME.as_ptr());
    instance_extensions.push(ash::khr::get_physical_device_properties2::NAME.as_ptr());
    instance_flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
  }

  let instance_info = vk::InstanceCreateInfo::default()
    .application_info(&app_info)
    .flags(instance_flags)
    .enabled_extension_names(&instance_extensions);
  let instance = unsafe { entry.create_instance(&instance_info, None)? };
  println!("Instance 创建成功");

  // ---------- 3. PhysicalDevice：枚举物理 GPU ----------
  // PhysicalDevice 代表一块真实（或转译）的 GPU，只能查询、不能直接使用。
  // 在 macOS 上枚举出来的就是 MoltenVK 包装的 Apple GPU（Metal 设备）。
  let physical_devices = unsafe { instance.enumerate_physical_devices()? };
  println!("找到 {} 个物理设备:", physical_devices.len());

  for &pdev in &physical_devices {
    let props = unsafe { instance.get_physical_device_properties(pdev) };
    let name = props.device_name_as_c_str().unwrap_or(c"unknown");
    println!(
      "  - {:?}  类型: {:?}  API: {}.{}.{}  驱动版本: {:#x}",
      name,
      props.device_type,
      vk::api_version_major(props.api_version),
      vk::api_version_minor(props.api_version),
      vk::api_version_patch(props.api_version),
      props.driver_version
    );
  }

  let physical_device = physical_devices[0];

  // ---------- 4. Queue Family：GPU 的命令通道 ----------
  // Vulkan 中所有工作（绘制、计算、传输）都以命令缓冲的形式提交到队列。
  // 队列按"族"划分，每个族支持不同的能力组合（GRAPHICS / COMPUTE / TRANSFER）。
  let queue_families =
    unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
  println!("队列族:");
  for (i, qf) in queue_families.iter().enumerate() {
    println!("  - 族 {}: {:?} x{}", i, qf.queue_flags, qf.queue_count);
  }
  let queue_family_index = queue_families
    .iter()
    .position(|qf| qf.queue_flags.contains(vk::QueueFlags::GRAPHICS))
    .ok_or("没有支持 GRAPHICS 的队列族")? as u32;

  // ---------- 5. 显存布局：Vulkan 把内存管理完全交给应用 ----------
  // 内存堆（heap）是物理显存/共享内存区域，内存类型（type）描述可见性与缓存属性。
  // Apple Silicon 是统一内存架构，通常只有一个 DEVICE_LOCAL 且 HOST_VISIBLE 的堆。
  let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
  println!("内存堆:");
  for (i, heap) in mem_props.memory_heaps[.. mem_props.memory_heap_count as usize]
    .iter()
    .enumerate()
  {
    println!(
      "  - 堆 {}: {} MB  {:?}",
      i,
      heap.size / 1024 / 1024,
      heap.flags
    );
  }

  // ---------- 6. Device + Queue：创建逻辑设备 ----------
  // Device（逻辑设备）是应用与 GPU 交互的主接口，后续所有资源（buffer、image、
  // pipeline、command buffer）都从它创建。创建时声明要用哪些队列和扩展。
  let queue_priorities = [1.0f32];
  let queue_info = vk::DeviceQueueCreateInfo::default()
    .queue_family_index(queue_family_index)
    .queue_priorities(&queue_priorities);

  // macOS: 若设备暴露了 VK_KHR_portability_subset，规范要求必须启用它。
  let mut device_extensions: Vec<*const std::os::raw::c_char> = Vec::new();
  let available_ext = unsafe { instance.enumerate_device_extension_properties(physical_device)? };
  if available_ext
    .iter()
    .any(|e| e.extension_name_as_c_str() == Ok(ash::khr::portability_subset::NAME))
  {
    device_extensions.push(ash::khr::portability_subset::NAME.as_ptr());
    println!("已启用 VK_KHR_portability_subset (MoltenVK 可移植性子集)");
  }

  let queue_infos = [queue_info];
  let device_info = vk::DeviceCreateInfo::default()
    .queue_create_infos(&queue_infos)
    .enabled_extension_names(&device_extensions);
  let device = unsafe { instance.create_device(physical_device, &device_info, None)? };
  let _queue = unsafe { device.get_device_queue(queue_family_index, 0) };
  println!("逻辑设备与队列创建成功");

  // ---------- 7. Buffer + Memory：显式的资源管理 ----------
  // 与 OpenGL 不同，Vulkan 中"创建 buffer"和"分配内存"是两步：
  // 先创建 buffer 对象，查询它的内存需求，再自己挑一种内存类型分配并绑定。
  // 这正是 Vulkan "低开销、显式控制"设计哲学的体现。
  let buffer_info = vk::BufferCreateInfo::default()
    .size(1024)
    .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
    .sharing_mode(vk::SharingMode::EXCLUSIVE);
  let buffer = unsafe { device.create_buffer(&buffer_info, None)? };

  let mem_req = unsafe { device.get_buffer_memory_requirements(buffer) };
  // 挑一个 CPU 可直接映射写入的内存类型（HOST_VISIBLE | HOST_COHERENT）
  let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
  let memory_type_index = (0 .. mem_props.memory_type_count)
    .find(|&i| {
      (mem_req.memory_type_bits & (1 << i)) != 0
        && mem_props.memory_types[i as usize]
          .property_flags
          .contains(wanted)
    })
    .ok_or("找不到 HOST_VISIBLE 内存类型")?;

  let alloc_info = vk::MemoryAllocateInfo::default()
    .allocation_size(mem_req.size)
    .memory_type_index(memory_type_index);
  let memory = unsafe { device.allocate_memory(&alloc_info, None)? };
  unsafe { device.bind_buffer_memory(buffer, memory, 0)? };

  // 映射到 CPU 地址空间并写入数据，证明整条链路可用
  unsafe {
    let ptr = device.map_memory(memory, 0, 1024, vk::MemoryMapFlags::empty())? as *mut u8;
    std::ptr::copy_nonoverlapping(b"hello vulkan on macos via MoltenVK".as_ptr(), ptr, 34);
    device.unmap_memory(memory);
  }
  println!("Buffer 创建、内存分配、映射写入全部成功");

  // ---------- 8. 清理：Vulkan 要求按创建的逆序显式销毁一切 ----------
  unsafe {
    device.destroy_buffer(buffer, None);
    device.free_memory(memory, None);
    device.destroy_device(None);
    instance.destroy_instance(None);
  }
  println!("资源清理完毕");
  Ok(())
}

// dlopen 默认搜索路径不包含 /opt/homebrew/lib，所以 Entry::load() 失败时
// 回退到 Homebrew 安装的 loader / MoltenVK 的绝对路径。
fn load_entry() -> Result<Entry, ash::LoadingError> {
  unsafe {
    Entry::load()
      .or_else(|_| Entry::load_from("/opt/homebrew/lib/libvulkan.dylib"))
      .or_else(|_| Entry::load_from("/usr/local/lib/libvulkan.dylib"))
      .or_else(|_| Entry::load_from("/opt/homebrew/lib/libMoltenVK.dylib"))
  }
}
