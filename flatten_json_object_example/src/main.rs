use flatten_json_object::ArrayFormatting;
use flatten_json_object::Flattener;
use serde_json::json;

fn main() {
    // println!("Hello, world!");

    let obj = json!({
        "a": {
            "b": [1, 2.0, "c", null, true, {}, []],
            "" : "my_key_is_empty"
        },
        "" : "my_key_is_also_empty"
    });
    assert_eq!(
        Flattener::new()
            .set_key_separator(".")
            .set_array_formatting(ArrayFormatting::Surrounded {
                start: "[".to_string(),
                end: "]".to_string()
            })
            .set_preserve_empty_arrays(false)
            .set_preserve_empty_objects(false)
            .flatten(&obj)
            .unwrap(),
        json!({
            "a.b[0]": 1,
            "a.b[1]": 2.0,
            "a.b[2]": "c",
            "a.b[3]": null,
            "a.b[4]": true,
            "a.": "my_key_is_empty",
            "": "my_key_is_also_empty",
        })
    );
}
