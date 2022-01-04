use std::borrow::Cow;

fn capitalize(name: &str) -> Cow<str> {
    match name.chars().nth(0) {
        Some(first_char) if first_char.is_uppercase() => {
            // No allocation is necessary, as the string
            // already starts with an uppercase char
            Cow::Borrowed(name)
        }
        Some(first_char) => {
            // An allocation is necessary, as the old string
            // does not start with an uppercase char
            let new_string: String = first_char
                .to_uppercase()
                .chain(name.chars().skip(1))
                .collect();

            Cow::Owned(new_string)
        }
        None => Cow::Borrowed(name),
    }
}

fn main() {
    println!("{}", capitalize("bob")); // Allocation
    println!("{}", capitalize("John")); // No allocation
}
