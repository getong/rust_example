use config::*;
use std::collections::HashMap;
use std::path::Path;

fn main() {
    // Option 1
    // --------
    // Gather all conf files from conf/ manually
    let mut settings = Config::default();
    settings
        // File::with_name(..) is shorthand for File::from(Path::new(..))
        .merge(File::from(Path::new("conf/sys.yml")))
        .unwrap();

    // Print out our settings (as a HashMap)
    println!(
        "\n{:?} \n\n-----------",
        settings.try_into::<HashMap<String, String>>().unwrap()
    );
}
