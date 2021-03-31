#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DVD {
    title: String,
    year: u32,
}

impl DVD {
    pub fn new(title: String, year: u32) -> Self {
        DVD { title, year }
    }
}

fn main() {
    let mut movies = vec![
        DVD::new("Buckaroo Banzai Across the 8th Dimension".to_string(), 1984),
        DVD::new("Captain America".to_string(), 2011),
        DVD::new("Stargate".to_string(), 1994),
        DVD::new("When Harry Met Sally".to_string(), 1989),
        DVD::new("Kiss Kiss Bang Bang".to_string(), 2005),
        DVD::new("The Dark Knight".to_string(), 2008),
        DVD::new("Boys Night Out".to_string(), 1962),
        DVD::new("The Glass Bottom Boat".to_string(), 1966),
    ];

    movies.sort_by(|a, b| b.year.cmp(&a.year));

    while let Some(movie) = movies.pop() {
        println!("{:?}", movie);
    }
}
