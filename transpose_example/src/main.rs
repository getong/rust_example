use transpose;

fn main() {
    // println!("Hello, world!");
    let input_array = vec![1, 2, 3, 4, 5, 6];

    // Treat our 6-element array as a 2D 3x2 array, and transpose it to a 2x3 array
    let mut output_array = vec![0; 6];
    transpose::transpose(&input_array, &mut output_array, 3, 2);

    // The rows have become the columns, and the columns have become the rows
    let expected_array = vec![1, 4, 2, 5, 3, 6];
    assert_eq!(output_array, expected_array);
}
