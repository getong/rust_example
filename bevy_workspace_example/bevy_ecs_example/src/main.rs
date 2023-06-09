use bevy::prelude::*;

#[derive(Component)]
struct Person;
#[derive(Component)]
struct Name(String);

fn add_people(mut commands: Commands) {
    commands.spawn((Person, Name("Rust".to_string())));

    commands.spawn((Person, Name("Bevy".to_string())));

    commands.spawn((Person, Name("Ferris".to_string())));
}

fn greet_people(query: Query<&Name, With<Person>>) {
    for name in query.iter() {
        println!("hello {}!", name.0);
    }
}

fn main() {
    App::new()
        .add_startup_system(add_people)
        .add_system(greet_people)
        .run();
}
