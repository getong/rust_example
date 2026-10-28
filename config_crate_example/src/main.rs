use std::{collections::HashMap, path::Path};

use config::*;

fn main() {
  // Option 1
  // --------
  // Gather all conf files from conf/ manually
  let settings = Config::builder()
    // File::with_name(..) is shorthand for File::from(Path::new(..))
    .add_source(File::from(Path::new("conf/sys.yml")))
    .build()
    .unwrap();

  // Print out our settings (as a HashMap)
  println!(
    "\n{:?} \n\n-----------",
    settings.try_deserialize::<HashMap<String, String>>().unwrap()
  );
}
