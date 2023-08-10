use clap::Parser;
use tokio::task::spawn;

#[derive(clap::Parser)]
struct CliArgs {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, clap::Subcommand)]
enum Command {
    /// Prints the current date and time.
    PrintDateTime,
    /// Spawns a new task that prints the current date and time.
    SpawnDateTime,
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

    match args.command {
        Command::PrintDateTime => {
            println!("{}", chrono::Local::now());
        }
        Command::SpawnDateTime => {
            spawn(async {
                loop {
                    println!("{}", chrono::Local::now());
                }
            });
        }
    }
}
