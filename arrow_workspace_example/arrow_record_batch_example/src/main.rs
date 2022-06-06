use std::sync::Arc;

use arrow::array::*;
use arrow::datatypes::*;
use arrow::error::Result as ArrowResult;
use arrow::record_batch::*;

fn main() -> ArrowResult<()> {
    // Define a schema with a vector of fields
    let schema = Schema::new(vec![
        Field::new("string", DataType::Utf8, false),
        Field::new("int", DataType::Int32, false),
        Field::new("float", DataType::Float64, false),
    ]);

    // Initialize primitive arrays with values
    let string_array = StringArray::from(vec!["one", "two", "three", "four", "five"]);
    let int_array = Int32Array::from(vec![1, 2, 3, 4, 5]);
    let float_array = Float64Array::from(vec![1.1, 2.2, 3.3, 4.4, 5.5]);

    // Build record batch with schema and arrays
    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(string_array),
            Arc::new(int_array),
            Arc::new(float_array),
        ])?;
   
    println!("{:?}", batch);

    Ok(())
}
