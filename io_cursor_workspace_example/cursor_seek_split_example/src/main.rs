struct Cursor<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Cursor { data, position: 0 }
    }

    fn seek(&mut self, offset: usize) {
        self.position = self.position.saturating_add(offset);
        if self.position > self.data.len() {
            self.position = self.data.len();
        }
    }

    fn split_at(&self, index: usize) -> (&'a [u8], &'a [u8]) {
        let first_part = &self.data[..index];
        let second_part = &self.data[index..];
        (first_part, second_part)
    }
}

fn main() {
    let bytes_vec: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    let mut cursor = Cursor::new(&bytes_vec);

    // Seek to a specific offset (e.g., 5 bytes).
    cursor.seek(5);

    // Split the vector at the cursor position.
    let (first_part, second_part) = cursor.split_at(cursor.position);

    // Print the two parts.
    println!("First part: {:?}", first_part);
    println!("Second part: {:?}", second_part);
}
