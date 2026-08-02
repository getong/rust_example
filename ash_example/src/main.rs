//! 一个使用 `ash` 直接调用 Vulkan 的最小初始化与内存示例。
//!
//! # `ash` 的功能和作用
//!
//! `ash` 是 Vulkan C API 的低层 Rust 绑定。它尽量保留 Vulkan 原有的对象模型和调用方式，
//! 主要负责：
//!
//! - 在 [`ash::vk`] 中提供 Vulkan 句柄、结构体、枚举和位标志的 Rust 表示；
//! - 通过 [`ash::Entry`] 加载 Vulkan loader，并取得全局级函数指针；
//! - 通过 [`ash::Instance`] 和 [`ash::Device`] 分别分发实例级、设备级 Vulkan 命令；
//! - 提供链式 setter，便于组装 `Vk*CreateInfo` 一类的调用参数。
//!
//! `ash` 本身不是 GPU 驱动或渲染引擎，也不会自动创建窗口、选择设备、管理显存、同步 GPU，
//! 或按依赖关系销毁资源。这些 Vulkan 约束仍由应用维护，所以大部分真正执行 Vulkan 命令的
//! 方法是 `unsafe`：Rust 编译器无法检查句柄是否有效、对象生命周期是否正确以及 GPU
//! 是否仍在使用资源。
//!
//! 本示例展示 `Entry -> Instance -> PhysicalDevice -> Device/Queue -> Buffer/Memory` 的基础链路，
//! 最终只向一块 CPU 可见的 Vulkan 内存写入字节。它没有创建 surface、swapchain、渲染管线或
//! command buffer，因此不会在屏幕上绘制图像。
//!
//! # Vulkan 的用途
//!
//! Vulkan 是 Khronos 制定的跨平台、低开销图形与通用 GPU 计算 API。与 OpenGL 相比，它把内存、
//! 同步和命令提交等工作显式交给应用控制，以换取更低的 CPU 开销和更可预测的性能。常见用途包括
//! 游戏与渲染引擎、compute shader、视频处理，以及 wgpu、Skia 等跨平台图形层的后端。
//!
//! # macOS 与 MoltenVK
//!
//! macOS 原生提供 Metal 而不是 Vulkan。MoltenVK 将 Vulkan 调用映射到 Metal，并以 Vulkan
//! portability implementation 的形式暴露设备。因此本示例在 macOS 上：
//!
//! 1. 创建 `Instance` 时启用 `VK_KHR_portability_enumeration`，并设置
//!    `ENUMERATE_PORTABILITY_KHR`，使 loader 可以枚举 MoltenVK 设备；
//! 2. 创建逻辑设备时，如果设备暴露 `VK_KHR_portability_subset`，则按规范启用该扩展。
//!
//! 可使用 `brew install molten-vk vulkan-loader` 安装本示例需要的运行库。

use std::error::Error;

use ash::{Entry, vk};

fn main() -> Result<(), Box<dyn Error>> {
  print_ash_role();

  // ---------- 1. Entry：加载 Vulkan 动态库 ----------
  // Entry 是 ash 与 Vulkan loader 的连接，持有创建 Instance、查询实例版本等全局级函数。
  // 启用 ash 默认的 `loaded` feature 后，这些函数通过 vkGetInstanceProcAddr 在运行时解析，
  // 应用不必在构建时直接链接某个具体 GPU 厂商的驱动。
  let entry = load_entry()?;
  print_entry_capabilities(&entry)?;

  // 这是 loader 支持的最高 Instance API 版本，不等同于某块物理设备支持的版本。
  let version = unsafe { entry.try_enumerate_instance_version()? }.unwrap_or(vk::API_VERSION_1_0);
  println!(
    "Vulkan instance 版本: {}.{}.{}",
    vk::api_version_major(version),
    vk::api_version_minor(version),
    vk::api_version_patch(version)
  );

  // ---------- 2. Instance：应用与 Vulkan 库的连接 ----------
  // Instance 记录本应用启用的 Vulkan 版本、扩展和 layer，并让 loader 连接适用的驱动（ICD）。
  // ash 会从 Instance 继续加载枚举物理设备等实例级函数。
  // ApplicationInfo 只描述应用意图和请求的 API 版本，并不负责创建 GPU 设备。
  let app_info = vk::ApplicationInfo::default()
    .application_name(c"ash_example")
    .application_version(vk::make_api_version(0, 1, 0, 0))
    .engine_name(c"no_engine")
    .api_version(vk::API_VERSION_1_2);

  // macOS（MoltenVK）必须启用 portability enumeration，才能把 portability
  // MoltenVK 设备枚举出来；VK_KHR_get_physical_device_properties2 是
  // portability_subset 设备扩展的依赖项。
  // 扩展名由 ash 以静态 CStr 常量提供；CreateInfo 保存的是这些字符串的原始指针。
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
  // 第二个参数为 None，表示让 Vulkan 使用默认的主机内存分配回调。
  let instance = unsafe { entry.create_instance(&instance_info, None)? };
  println!("Instance 创建成功");

  // ---------- 3. PhysicalDevice：枚举物理 GPU ----------
  // vk::PhysicalDevice 是由 loader/驱动返回的不透明句柄，代表一块真实或转译的 GPU。
  // 它用于查询能力；真正创建资源和提交命令需要从它创建逻辑 Device。
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

  // 作为最小示例直接选择第一块设备；实际引擎通常会按扩展、队列和显存能力评分选择。
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
  // 内存堆（heap）表示实际的物理显存或共享内存容量；内存类型（type）把堆与
  // DEVICE_LOCAL、HOST_VISIBLE、HOST_COHERENT 等访问属性关联起来。
  // Apple Silicon 采用统一内存架构，设备本地内存类型通常也可以对 CPU 可见。
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
  // Device（逻辑设备）是应用与 GPU 交互的主接口。ash::Device 持有设备级函数表，
  // buffer、image、pipeline、command buffer 等资源都通过它创建。创建 Device 时需要
  // 预先声明将使用的队列和设备扩展。
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
  // Queue 是 Device 拥有的非独占句柄，无需单独销毁；销毁 Device 时一并失效。
  let _queue = unsafe { device.get_device_queue(queue_family_index, 0) };
  println!("逻辑设备与队列创建成功");

  // ---------- 7. Buffer + Memory：显式的资源管理 ----------
  // 与 OpenGL 不同，Vulkan 中"创建 buffer"和"分配内存"是两步：
  // 先创建 buffer 对象，查询它的内存需求，再挑选兼容的内存类型进行分配和绑定。
  // ash 只转发这些 Vulkan 操作，不包含显存分配器；大型程序通常在其上使用专门的 allocator。
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

  // HOST_COHERENT 保证解除映射前不必再显式 flush CPU 写入。
  // 映射到 CPU 地址空间并写入数据，证明整条链路可用。
  unsafe {
    let ptr = device.map_memory(memory, 0, 1024, vk::MemoryMapFlags::empty())? as *mut u8;
    std::ptr::copy_nonoverlapping(b"hello vulkan on macos via MoltenVK".as_ptr(), ptr, 34);
    device.unmap_memory(memory);
  }
  println!("Buffer 创建、内存分配、映射写入全部成功");

  // ---------- 8. 清理：ash 不替 Vulkan 句柄自动实现 Drop ----------
  // 本例没有提交 GPU 工作，因此无需等待队列空闲。按依赖关系逆序销毁资源：
  // 先销毁绑定到 memory 的 buffer，再释放 memory，最后销毁 Device 和 Instance。
  unsafe {
    device.destroy_buffer(buffer, None);
    device.free_memory(memory, None);
    device.destroy_device(None);
    instance.destroy_instance(None);
  }
  println!("资源清理完毕");
  Ok(())
}

/// 输出 `ash` 在 Vulkan 程序中的职责和边界。
fn print_ash_role() {
  println!("ash 的功能和作用:");
  println!("  - ash::vk: 提供 Vulkan 类型、常量、句柄和创建参数");
  println!("  - ash::Entry: 加载 Vulkan loader 和全局级函数");
  println!("  - ash::Instance: 调用实例级函数，例如枚举物理设备");
  println!("  - ash::Device: 调用设备级函数，例如创建 Buffer 和提交命令");
  println!("  - 应用程序: 负责选择设备、同步、显存和资源生命周期");
  println!();
}

/// 使用 `ash::Entry` 查询并输出 Vulkan loader 当前暴露的实例能力。
///
/// 这段代码演示 `ash` 不会虚构或模拟 Vulkan 功能，而是调用 loader 的
/// `vkEnumerateInstanceExtensionProperties` 和 `vkEnumerateInstanceLayerProperties`，
/// 再把返回的 C 结构体转换成便于 Rust 代码读取的 [`vk`] 类型。
///
/// # Errors
///
/// 当 Vulkan loader 无法完成扩展或 layer 枚举时，返回对应的 [`vk::Result`]。
fn print_entry_capabilities(entry: &Entry) -> Result<(), vk::Result> {
  // SAFETY: Entry 持有有效的全局级函数表；ash 在调用期间管理枚举结果的输出缓冲区。
  let extensions = unsafe { entry.enumerate_instance_extension_properties(None)? };
  println!("ash::Entry 查询到 {} 个实例扩展:", extensions.len());
  for extension in &extensions {
    let name = extension
      .extension_name_as_c_str()
      .unwrap_or(c"<invalid extension name>");
    println!(
      "  - {} (spec {})",
      name.to_string_lossy(),
      extension.spec_version
    );
  }

  // SAFETY: 与上面的扩展查询相同，Entry 和 loader 函数表在整个调用期间保持有效。
  let layers = unsafe { entry.enumerate_instance_layer_properties()? };
  println!("ash::Entry 查询到 {} 个实例 layer:", layers.len());
  for layer in &layers {
    let name = layer
      .layer_name_as_c_str()
      .unwrap_or(c"<invalid layer name>");
    println!("  - {}", name.to_string_lossy());
  }
  println!();

  Ok(())
}

/// 加载 Vulkan loader，并在默认动态库搜索失败时尝试 Homebrew 路径。
///
/// `Entry` 负责持有动态库及其全局级函数表。优先加载 `libvulkan` 可以让 Vulkan loader
/// 统一发现系统中的 ICD；最后直接加载 MoltenVK 仅作为 macOS 上的本地回退路径。
fn load_entry() -> Result<Entry, ash::LoadingError> {
  unsafe {
    Entry::load()
      .or_else(|_| Entry::load_from("/opt/homebrew/lib/libvulkan.dylib"))
      .or_else(|_| Entry::load_from("/usr/local/lib/libvulkan.dylib"))
      .or_else(|_| Entry::load_from("/opt/homebrew/lib/libMoltenVK.dylib"))
  }
}
