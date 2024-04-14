use specs::{Builder, Component, ReadStorage, RunNow, System, VecStorage, World, WorldExt};

#[derive(Debug)]
pub struct Position {
  pub x: f32,
  pub y: f32,
}

impl Component for Position {
  type Storage = VecStorage<Self>;
}

#[derive(Debug)]
pub struct Velocity {
  pub x: f32,
  pub y: f32,
}

impl Component for Velocity {
  type Storage = VecStorage<Self>;
}

struct HelloWorld;

impl<'a> System<'a> for HelloWorld {
  type SystemData = ReadStorage<'a, Position>;

  fn run(&mut self, position: Self::SystemData) {
    use specs::Join;

    for position in position.join() {
      println!("Hello, {:?}", &position);
    }
  }
}

fn main() {
  let mut world = World::new();
  world.register::<Position>();
  world.register::<Velocity>();

  world
    .create_entity()
    .with(Position { x: 4.0, y: 7.0 })
    .build();

  let mut hello_world = HelloWorld;
  hello_world.run_now(&world);
  world.maintain();
}
