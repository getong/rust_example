use specs::prelude::*;

#[derive(Default)]
struct Gravity;

struct Velocity;

impl Component for Velocity {
    type Storage = VecStorage<Self>;
}

struct SimulationSystem;

impl<'a> System<'a> for SimulationSystem {
    type SystemData = (Read<'a, Gravity>, WriteStorage<'a, Velocity>);

    fn run(&mut self, _: Self::SystemData) {}
}

fn main() {
    //    let mut world = World::new();
    //    world.insert(Gravity);
    //    world.register::<Velocity>();
    //
    //    for _ in 0..5 {
    //        world.create_entity().with(Velocity).build();
    //    }
    //
    //    let mut dispatcher = DispatcherBuilder::new()
    //        .with(SimulationSystem, "simulation", &[])
    //        .build();
    //
    //    dispatcher.dispatch(&mut world);
    //    world.maintain();

    // The code below is equal as the above
    let mut world = World::new();
    let mut dispatcher = DispatcherBuilder::new()
        .with(SimulationSystem, "simulation", &[])
        .build();

    dispatcher.setup(&mut world);

    for _ in 0..5 {
        world.create_entity().with(Velocity).build();
    }

    dispatcher.dispatch(&mut world);
    world.maintain();
}
