fn main() {
    // println!("Hello, world!");
    let logical_cpus = num_cpus::get();
    let physical_cpus = num_cpus::get_physical();
    println!(
        "logical_cpus:{}, physical_cpus:{}",
        logical_cpus, physical_cpus
    );

    if logical_cpus > physical_cpus {
        println!(
            "We have simultaneous multithreading with about {:.2} \
              logical cores to 1 physical core.",
            (logical_cpus as f64) / (physical_cpus as f64)
        );
    } else if logical_cpus == physical_cpus {
        println!(
            "Either we don't have simultaneous multithreading, or our \
              system doesn't support getting the number of physical CPUs."
        );
    } else {
        println!(
            "We have less logical CPUs than physical CPUs, maybe we only have access to \
              some of the CPUs on our system."
        );
    }
}
