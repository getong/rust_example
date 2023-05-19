use error_chain::error_chain;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

// change the directory you want to modify
const FILE_PATH: &[u8] = b"~/rust/";

error_chain! {
    foreign_links {
        Io(std::io::Error);
        SystemTimeError(std::time::SystemTimeError);
    }
}

fn main() -> Result<()> {
    let dir_name =
        String::from_utf8(tilde_expand::tilde_expand(FILE_PATH)).expect("Invalid UTF-8 sequence");

    let current_dir = PathBuf::from(dir_name);

    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        let metadata = fs::metadata(&path)?;

        if metadata.is_dir() {
            // println!("path : {:?}", path);
            let sub_paths = fs::read_dir(path.clone())?;

            let mut sub_dir_count = 0;
            let mut sub_dir_name: PathBuf = PathBuf::new();
            for sub_entry in sub_paths {
                let sub_entry = sub_entry?;
                let sub_path = sub_entry.path();
                let sub_metadata = fs::metadata(&sub_path)?;
                if sub_metadata.is_dir() {
                    sub_dir_count += 1;
                    sub_dir_name = sub_path.clone();
                }
            }
            if sub_dir_count == 1 {
                // println!(
                //     "path {} has only one  dirs, {}",
                //     sub_path.display(),
                //     sub_dir_name.display()
                // );
                _ = move_directory(&sub_dir_name, &path);
            }
        }
    }

    Ok(())
}

fn move_directory(source: &PathBuf, destination: &Path) -> std::io::Result<()> {
    // Read the entries in the source directory
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let entry_path = entry.path();

        let new_path = destination.join(entry.file_name());
        fs::rename(&entry_path, &new_path)?;
    }
    fs::remove_dir(source)?;
    println!(
        "source: {:?}, destination: {:?}",
        source.display(),
        destination.display()
    );

    Ok(())
}
