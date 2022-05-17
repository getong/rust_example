use std::fs;

fn main() {
    let metadata = fs::symlink_metadata("foo.txt").unwrap();
    let file_type = metadata.file_type();

    let result = file_type.is_symlink();
    println!("result: {}", result);
}
