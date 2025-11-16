use std::{cmp, thread, time::Duration};

use many_cpus::{HardwareInfo, HardwareTracker, ProcessorSet};
use sysinfo::{Disks, MINIMUM_CPU_UPDATE_INTERVAL, System};

fn main() {
  print_folo_hardware_snapshot();
  println!();
  print_live_system_metrics();
}

fn print_folo_hardware_snapshot() {
  println!("=== Folo hardware snapshot ===");

  let max_processors = HardwareInfo::max_processor_count();
  let max_regions = HardwareInfo::max_memory_region_count();
  println!(
    "Hardware supports up to {max_processors} processor(s) across {max_regions} memory region(s)."
  );

  let processors = ProcessorSet::default();
  println!(
    "Processors currently available to this process: {}",
    processors.len()
  );

  for processor in processors.processors().iter().take(8) {
    println!(
      "  Processor {:>3}: region {:>2}, class {:?}",
      processor.id(),
      processor.memory_region_id(),
      processor.efficiency_class()
    );
  }

  if processors.len() > 8 {
    println!(
      "  ... {} more processor(s) omitted ...",
      processors.len() - 8
    );
  }

  HardwareTracker::with_current_processor(|processor| {
    println!(
      "Current thread runs on processor {} (region {}) with {:?} efficiency",
      processor.id(),
      processor.memory_region_id(),
      processor.efficiency_class()
    );
  });
}

fn print_live_system_metrics() {
  println!("=== Live system metrics ===");

  let mut system = System::new_all();
  // Refresh CPU usage twice with a short delay to get meaningful values.
  system.refresh_cpu_all();
  let pause = cmp::max(MINIMUM_CPU_UPDATE_INTERVAL, Duration::from_millis(200));
  thread::sleep(pause);
  system.refresh_cpu_usage();

  system.refresh_memory();
  let mut disks = Disks::new_with_refreshed_list();
  disks.refresh(false);

  let total_memory = system.total_memory();
  let used_memory = system.used_memory();
  let free_memory = system.free_memory();
  println!(
    "Memory: used {} / total {} (free {})",
    format_bytes(used_memory),
    format_bytes(total_memory),
    format_bytes(free_memory)
  );

  let cpus = system.cpus();
  let average_usage = if cpus.is_empty() {
    0.0
  } else {
    cpus
      .iter()
      .map(|cpu| f64::from(cpu.cpu_usage()))
      .sum::<f64>()
      / cpus.len() as f64
  };
  println!(
    "CPU usage across {} logical processor(s): {:.1}% avg",
    cpus.len(),
    average_usage
  );
  for (index, cpu) in cpus.iter().enumerate().take(4) {
    println!(
      "  CPU {:>2} ({:>4}) -> {:>5.1}% @ {} MHz",
      index,
      cpu.name(),
      cpu.cpu_usage(),
      cpu.frequency()
    );
  }
  if cpus.len() > 4 {
    println!("  ... {} more CPU(s) omitted ...", cpus.len() - 4);
  }

  let disks = disks.list();
  if disks.is_empty() {
    println!("No disks reported by sysinfo.");
  } else {
    println!("Disk usage:");
    for disk in disks {
      let name = disk.name().to_string_lossy();
      let fs = disk.file_system().to_string_lossy();
      let total = disk.total_space();
      let available = disk.available_space();
      let used = total.saturating_sub(available);
      println!(
        "  {name}: used {} / {} (fs: {fs})",
        format_bytes(used),
        format_bytes(total)
      );
    }
  }
}

fn format_bytes(value: u64) -> String {
  const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
  let mut value = value as f64;
  let mut unit_index = 0;

  while value >= 1024.0 && unit_index < UNITS.len() - 1 {
    value /= 1024.0;
    unit_index += 1;
  }

  format!("{value:.2} {}", UNITS[unit_index])
}
