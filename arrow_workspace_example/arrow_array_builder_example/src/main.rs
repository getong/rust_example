use arrow::array::{Array, Int32Array, Int32Builder};
use rand::distributions::{Distribution, Uniform};

fn main() {
    // Array from vector
    let primitive_array = array();

    println!("{:?}", primitive_array);

    // Array from builder
    let primitive_array_from_builder = array_builder();

    println!("{:?}", primitive_array_from_builder);
}

fn array() -> Int32Array {
    // Convert vector of Option types to array
    let array = Int32Array::from(vec![Some(1), None, Some(3), None, Some(5)]);
    assert_eq!(array.len(), 5);
    assert_eq!(array.value(0), 1);
    assert_eq!(array.is_null(1), true);

    array
}

fn array_builder() -> Int32Array {
    let range = Uniform::from(1..100);
    let mut rng = rand::thread_rng();

    // Initialize array builder
    let mut primitive_array_builder = Int32Builder::new(100);

    // Randomly gnerate data and append it to the array
    for _ in 0..50 {
        let value = range.sample(&mut rng);

        if value % 2 == 0 {
            primitive_array_builder.append_value(value).unwrap();
        } else {
            primitive_array_builder.append_null().unwrap();
        }
    }

    let values = (0..50)
        .map(|_| range.sample(&mut rng))
        .collect::<Vec<i32>>();

    // Append slice of values to array
    primitive_array_builder.append_slice(&values).unwrap();

    // Consume builder and convert to array
    primitive_array_builder.finish()
}
