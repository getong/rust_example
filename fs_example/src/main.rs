use std::fs;
use std::fs::File;
use std::fs::OpenOptions;

fn main() {
    println!("Hello, world!");

    // create file
    File::create("./stats.txt");

    // open file
    File::open("stats1.txt").expect("File not found");

    // open file
    OpenOptions::new()
        .write(true)
        .create(true)
        .open("stats2.txt");

    // copy file
    fs::copy("stats1.txt", "stats2.txt").expect("Unable to copy");

    // rename file
    fs::rename("stats1.txt", "stats3.txt").expect("Unable to rename");

    // read file
    let byte_arr = fs::read("stats3.txt").expect("Unable to read file into bytes");
    println!(
        "Value read from file into bytes is {}",
        String::from_utf8(byte_arr).unwrap()
    );

    let string1 = fs::read_to_string("stats3.txt").expect("Unable to read file into string");
    println!("Value read from file into string is {}", string1);

    // write file
    fs::write("stats3.txt", "Rust is exciting,isn't it?").expect("Unable to write to file");

    // metadata
    let file_metadata = fs::metadata("stats.txt").expect("Unable to get file metadata");
    println!(
        "Len: {}, last accessed: {:?}, modified : {:?}, created: {:?}",
        file_metadata.len(),
        file_metadata.accessed(),
        file_metadata.modified(),
        file_metadata.created()
    );

    println!(
        "Is file: {}, Is dir: {}, is Symlink: {}",
        file_metadata.is_file(),
        file_metadata.is_dir(),
        file_metadata.file_type().is_symlink()
    );

    println!("File metadata: {:?}", fs::metadata("stats.txt"));
    println!("Permissions of file are: {:?}", file_metadata.permissions());

    // set permission
    let mut permissions = fs::metadata("stats.txt").unwrap().permissions();
    permissions.set_readonly(true);
    let _ = fs::set_permissions("stats.txt", permissions).expect("Unable to set permission");
    fs::write("stats.txt", "Hello- Can you see me?").expect("Unable to write to file");
}
