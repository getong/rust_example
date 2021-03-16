use std::collections::BinaryHeap;

fn show_remainder(input: &BinaryHeap<i32>) -> Vec<i32> {
    // This function shows the remainder in the BinaryHeap. Actually an iterator would be
    // faster than a function - we will learn them later.
    let mut remainder_vec = vec![];
    for number in input {
        remainder_vec.push(*number)
    }
    remainder_vec
}

fn main() {
    let many_numbers = vec![0, 5, 10, 15, 20, 25, 30]; // These numbers are in order

    let mut my_heap = BinaryHeap::new();

    for number in many_numbers {
        my_heap.push(number);
    }

    while let Some(number) = my_heap.pop() {
        // .pop() returns Some(number) if a number is there, None if not. It pops from the front
        println!(
            "Popped off {}. Remaining numbers are: {:?}",
            number,
            show_remainder(&my_heap)
        );
    }

    let mut jobs = BinaryHeap::new();
    // Add jobs to do throughout the day
    jobs.push((100, "Write back to email from the CEO"));
    jobs.push((80, "Finish the report today"));
    jobs.push((5, "Watch some YouTube"));
    jobs.push((70, "Tell your team members thanks for always working hard"));
    jobs.push((30, "Plan who to hire next for the team"));
    while let Some(job) = jobs.pop() {
        println!("You need to: {}", job.1);
    }
}
