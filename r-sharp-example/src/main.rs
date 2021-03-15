fn r#return() -> u8 {
    println!("Here is your number.");
    8
}

fn main() {
    println!("He said, \"You can find the file at c:\\files\\my_documents\\file.txt.\" Then I found the file."); // We used \ five times here
    println!(
        r#"He said, "You can find the file at c:\files\my_documents\file.txt." Then I found the file."#
    );

    let my_string = "'Ice to see you,' he said."; // single quotes
    let quote_string = r#""Ice to see you," he said."#; // double quotes
    let hashtag_string = r##"The hashtag #IceToSeeYou had become very popular."##; // Has one # so we need at least ##
    let many_hashtags =
        r####""You don't have to type ### to use a hashtag. You can just use #.""####; // Has three ### so we need at least ####

    println!(
        "{}\n{}\n{}\n{}\n",
        my_string, quote_string, hashtag_string, many_hashtags
    );

    let r#let = 6; // The variable's name is let
    let mut r#mut = 10; // This variable's name is mut
    println!("r#let is {}, r#mut is {}", r#let, r#mut);

    let my_number = r#return();
    println!("{}", my_number);

    println!("{:?}", b"This will look like numbers");

    println!("{:?}", br##"I like to write "#"."##);

    println!("{:X}", '행' as u32); // Cast char as u32 to get the hexadecimal value
    println!("{:X}", 'H' as u32);
    println!("{:X}", '居' as u32);
    println!("{:X}", 'い' as u32);

    println!("\u{D589}, \u{48}, \u{5C45}, \u{3044}"); // Try printing them with unicode escape \u
}
