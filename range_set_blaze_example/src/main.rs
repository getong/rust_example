use range_set_blaze::RangeSetBlaze;
fn main() {
    // println!("Hello, world!");

    // a is the set of integers from 100 to 499 (inclusive) and 501 to 1000 (inclusive)
    let a = RangeSetBlaze::from_iter([100..=499, 501..=999]);
    // b is the set of integers -20 and the range 400 to 599 (inclusive)
    let b = RangeSetBlaze::from_iter([-20..=-20, 400..=599]);
    // c is the union of a and b, namely -20 and 100 to 999 (inclusive)
    let c = a | b;
    assert_eq!(c, RangeSetBlaze::from_iter([-20..=-20, 100..=999]));

    let line = "chr15   29370   37380   29370,32358,36715   30817,32561,37380";

    // split the line on white space
    let mut iter = line.split_whitespace();
    let chr = iter.next().unwrap();

    // Parse the start and end of the transcription region into a RangeSetBlaze
    let trans_start: i32 = iter.next().unwrap().parse().unwrap();
    let trans_end: i32 = iter.next().unwrap().parse().unwrap();
    let trans = RangeSetBlaze::from_iter([trans_start..=trans_end]);
    assert_eq!(trans, RangeSetBlaze::from_iter([29370..=37380]));

    // Parse the start and end of the exons into a RangeSetBlaze
    let exon_starts = iter.next().unwrap().split(',').map(|s| s.parse::<i32>());
    let exon_ends = iter.next().unwrap().split(',').map(|s| s.parse::<i32>());
    let exon_ranges = exon_starts
        .zip(exon_ends)
        .map(|(s, e)| s.unwrap()..=e.unwrap());
    let exons = RangeSetBlaze::from_iter(exon_ranges);
    assert_eq!(
        exons,
        RangeSetBlaze::from_iter([29370..=30817, 32358..=32561, 36715..=37380])
    );

    // Use 'set difference' to find the introns
    let intron = trans - exons;
    assert_eq!(
        intron,
        RangeSetBlaze::from_iter([30818..=32357, 32562..=36714])
    );
    for range in intron.ranges() {
        let (start, end) = range.into_inner();
        println!("{chr}\t{start}\t{end}");
    }
}
