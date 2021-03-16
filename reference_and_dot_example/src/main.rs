struct Item {
    number: u8,
}

impl Item {
    fn compare_number(&self, other_number: u8) {
        // takes a reference to self
        println!(
            "Are {} and {} equal? {}",
            self.number,
            other_number,
            self.number == other_number
        );
        // We don't need to write *self.number
    }
}

fn main() {
    let my_number = 9;
    let reference = &my_number;

    println!("{}", my_number == *reference);

    let item = Item { number: 8 };

    let reference_number = &item.number; // reference number type is &u8

    println!("{}", *reference_number == 8); // ⚠️ &u8 and u8 cannot be compared

    let item = Item { number: 8 };

    let reference_item = &item; // This is type &Item
    let reference_item_two = &reference_item; // This is type &&Item

    item.compare_number(8); // the method works
    reference_item.compare_number(8); // it works here too
    reference_item_two.compare_number(8); // and here
}
