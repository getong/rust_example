use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use anyhow::Result;
use wasmtime::{Config, Engine, Instance, Module, ResourceLimiter, Store, Trap};

const WASM_PAGE_SIZE: usize = 64 * 1024;

fn main() -> Result<()> {
  println!("Wasmtime sandbox limits demo");
  println!("================================");

  fuel_allows_bounded_work()?;
  fuel_stops_infinite_loop()?;
  epoch_stops_infinite_loop()?;
  memory_limiter_blocks_growth()?;
  table_limiter_blocks_growth()?;
  instance_limiter_blocks_extra_instances()?;

  Ok(())
}

fn fuel_allows_bounded_work() -> Result<()> {
  println!("\n1. fuel: bounded work completes and consumes fuel");

  let engine = metered_engine(true, false)?;
  let module = Module::new(
    &engine,
    r#"
      (module
        (func (export "sum_to") (param $n i32) (result i32)
          (local $sum i32)
          (block $done
            (loop $again
              local.get $n
              i32.eqz
              br_if $done

              local.get $sum
              local.get $n
              i32.add
              local.set $sum

              local.get $n
              i32.const 1
              i32.sub
              local.set $n

              br $again))
          local.get $sum))
    "#,
  )?;
  let mut store = Store::new(&engine, ());
  let fuel_budget = 50_000;
  store.set_fuel(fuel_budget)?;

  let instance = Instance::new(&mut store, &module, &[])?;
  let sum_to = instance.get_typed_func::<i32, i32>(&mut store, "sum_to")?;
  let result = sum_to.call(&mut store, 100)?;
  let remaining = store.get_fuel()?;

  println!("   sum_to(100) = {result}");
  println!(
    "   fuel budget={fuel_budget}, consumed={}, remaining={remaining}",
    fuel_budget - remaining
  );

  Ok(())
}

fn fuel_stops_infinite_loop() -> Result<()> {
  println!("\n2. fuel: infinite loop traps after the budget is exhausted");

  let engine = metered_engine(true, false)?;
  let module = spinning_module(&engine)?;
  let mut store = Store::new(&engine, ());
  store.set_fuel(10_000)?;

  let instance = Instance::new(&mut store, &module, &[])?;
  let spin = instance.get_typed_func::<(), ()>(&mut store, "spin")?;
  let err = spin
    .call(&mut store, ())
    .expect_err("spin should run out of fuel");

  print_trap("   trap", &err);
  println!("   remaining fuel={}", store.get_fuel()?);

  Ok(())
}

fn epoch_stops_infinite_loop() -> Result<()> {
  println!("\n3. epoch interruption: wall-clock backstop interrupts running wasm");

  let engine = metered_engine(false, true)?;
  let module = spinning_module(&engine)?;
  let mut store = Store::new(&engine, ());
  store.epoch_deadline_trap();
  store.set_epoch_deadline(1);

  let stop_ticker = Arc::new(AtomicBool::new(false));
  let ticker = spawn_epoch_ticker(
    engine.clone(),
    stop_ticker.clone(),
    Duration::from_millis(10),
  );

  let instance = Instance::new(&mut store, &module, &[])?;
  let spin = instance.get_typed_func::<(), ()>(&mut store, "spin")?;

  let started = Instant::now();
  let err = spin
    .call(&mut store, ())
    .expect_err("spin should be interrupted by the epoch deadline");
  stop_ticker.store(true, Ordering::Relaxed);
  ticker
    .join()
    .expect("epoch ticker thread should join cleanly");

  print_trap("   trap", &err);
  println!("   elapsed roughly {:?}", started.elapsed());

  Ok(())
}

fn memory_limiter_blocks_growth() -> Result<()> {
  println!("\n4. memory limiter: memory.grow is denied past 2 wasm pages");

  let engine = metered_engine(false, false)?;
  let module = Module::new(
    &engine,
    r#"
      (module
        (memory 1 10)
        (func (export "grow_memory") (param $pages i32) (result i32)
          local.get $pages
          memory.grow))
    "#,
  )?;
  let mut store = Store::new(
    &engine,
    StoreState::with_limiter(
      SandboxLimiter::new()
        .with_memory_bytes(2 * WASM_PAGE_SIZE)
        .with_table_elements(10_000)
        .with_instances(10),
    ),
  );
  store.limiter(|state| &mut state.limiter);

  let instance = Instance::new(&mut store, &module, &[])?;
  let grow_memory = instance.get_typed_func::<i32, i32>(&mut store, "grow_memory")?;

  let first = grow_memory.call(&mut store, 1)?;
  let second = grow_memory.call(&mut store, 1)?;

  println!("   grow by 1 page from 1 page -> returned {first} (success)");
  println!("   grow by 1 more page past limit -> returned {second} (-1 means denied)");
  store.data().limiter.print_recent_events("   limiter");

  Ok(())
}

fn table_limiter_blocks_growth() -> Result<()> {
  println!("\n5. table limiter: table.grow is denied past 3 elements");

  let engine = metered_engine(false, false)?;
  let module = Module::new(
    &engine,
    r#"
      (module
        (table 1 100 funcref)
        (func (export "grow_table") (param $slots i32) (result i32)
          ref.null func
          local.get $slots
          table.grow))
    "#,
  )?;
  let mut store = Store::new(
    &engine,
    StoreState::with_limiter(
      SandboxLimiter::new()
        .with_memory_bytes(10 * WASM_PAGE_SIZE)
        .with_table_elements(3)
        .with_instances(10),
    ),
  );
  store.limiter(|state| &mut state.limiter);

  let instance = Instance::new(&mut store, &module, &[])?;
  let grow_table = instance.get_typed_func::<i32, i32>(&mut store, "grow_table")?;

  let first = grow_table.call(&mut store, 2)?;
  let second = grow_table.call(&mut store, 1)?;

  println!("   grow table by 2 from size 1 -> returned {first} (success)");
  println!("   grow by 1 more slot past limit -> returned {second} (-1 means denied)");
  store.data().limiter.print_recent_events("   limiter");

  Ok(())
}

fn instance_limiter_blocks_extra_instances() -> Result<()> {
  println!("\n6. instance limiter: second instance in one Store is rejected");

  let engine = metered_engine(false, false)?;
  let module = Module::new(
    &engine,
    r#"
      (module
        (func (export "answer") (result i32)
          i32.const 42))
    "#,
  )?;
  let mut store = Store::new(
    &engine,
    StoreState::with_limiter(
      SandboxLimiter::new()
        .with_memory_bytes(10 * WASM_PAGE_SIZE)
        .with_table_elements(10_000)
        .with_instances(1),
    ),
  );
  store.limiter(|state| &mut state.limiter);

  let first = Instance::new(&mut store, &module, &[]);
  println!("   first instantiate succeeded={}", first.is_ok());

  let second = Instance::new(&mut store, &module, &[]);
  match second {
    Ok(_) => println!("   second instantiate unexpectedly succeeded"),
    Err(err) => println!("   second instantiate failed as expected: {err}"),
  }

  Ok(())
}

fn metered_engine(consume_fuel: bool, epoch_interruption: bool) -> Result<Engine> {
  let mut config = Config::new();
  config.consume_fuel(consume_fuel);
  config.epoch_interruption(epoch_interruption);
  Ok(Engine::new(&config)?)
}

fn spinning_module(engine: &Engine) -> Result<Module> {
  Ok(Module::new(
    engine,
    r#"
      (module
        (func (export "spin")
          (loop $again
            br $again)))
    "#,
  )?)
}

fn spawn_epoch_ticker(
  engine: Engine,
  stop: Arc<AtomicBool>,
  interval: Duration,
) -> thread::JoinHandle<()> {
  thread::Builder::new()
    .name("wasmtime-example-epoch-ticker".to_owned())
    .spawn(move || {
      while !stop.load(Ordering::Relaxed) {
        thread::sleep(interval);
        engine.increment_epoch();
      }
    })
    .expect("failed to spawn epoch ticker")
}

fn print_trap(prefix: &str, err: &wasmtime::Error) {
  if let Some(trap) = err.downcast_ref::<Trap>() {
    println!("{prefix}: {trap:?}");
  } else {
    println!("{prefix}: {err}");
  }
}

#[derive(Debug)]
struct StoreState {
  limiter: SandboxLimiter,
}

impl StoreState {
  fn with_limiter(limiter: SandboxLimiter) -> Self {
    Self { limiter }
  }
}

#[derive(Debug)]
struct SandboxLimiter {
  memory_limit_bytes: usize,
  table_limit_elements: usize,
  max_instances: usize,
  max_tables: usize,
  max_memories: usize,
  events: Vec<String>,
}

impl SandboxLimiter {
  fn new() -> Self {
    Self {
      memory_limit_bytes: 10 * WASM_PAGE_SIZE,
      table_limit_elements: 10_000,
      max_instances: 10,
      max_tables: 10,
      max_memories: 10,
      events: Vec::new(),
    }
  }

  fn with_memory_bytes(mut self, bytes: usize) -> Self {
    self.memory_limit_bytes = bytes;
    self
  }

  fn with_table_elements(mut self, elements: usize) -> Self {
    self.table_limit_elements = elements;
    self
  }

  fn with_instances(mut self, instances: usize) -> Self {
    self.max_instances = instances;
    self
  }

  fn print_recent_events(&self, prefix: &str) {
    for event in &self.events {
      println!("{prefix}: {event}");
    }
  }
}

impl ResourceLimiter for SandboxLimiter {
  fn memory_growing(
    &mut self,
    current: usize,
    desired: usize,
    maximum: Option<usize>,
  ) -> wasmtime::Result<bool> {
    let allowed = desired <= self.memory_limit_bytes;
    self.events.push(format!(
      "memory_growing current={}B desired={}B max={:?} limit={}B allowed={allowed}",
      current, desired, maximum, self.memory_limit_bytes
    ));
    Ok(allowed)
  }

  fn table_growing(
    &mut self,
    current: usize,
    desired: usize,
    maximum: Option<usize>,
  ) -> wasmtime::Result<bool> {
    let allowed = desired <= self.table_limit_elements;
    self.events.push(format!(
      "table_growing current={current} desired={desired} max={maximum:?} limit={} \
       allowed={allowed}",
      self.table_limit_elements
    ));
    Ok(allowed)
  }

  fn instances(&self) -> usize {
    self.max_instances
  }

  fn tables(&self) -> usize {
    self.max_tables
  }

  fn memories(&self) -> usize {
    self.max_memories
  }
}
