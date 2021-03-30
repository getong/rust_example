use sqlite;
use sqlite::State;
use sqlite::Connection;
use std::io;

pub fn addrecord(conn: &Connection) -> io::Result<()> {
    let mut title = String::new();
    let mut finding = String::new();
    let mut details = String::new();
    let mut justification = String::new();

    println!("Title");
    io::stdin().read_line(&mut title)?;
    println!("Finding text");
    io::stdin().read_line(&mut finding)?;
    println!("Details of the finding");
    io::stdin().read_line(&mut details)?;
    println!("Justification");
    io::stdin().read_line(&mut justification)?;

    let commandstring = format!("INSERT INTO findings (title, finding, details, justification) VALUES (\"{}\",
        \"{}\", \"{}\", \"{}\")", title.trim(), finding.trim(), details.trim(), justification.trim());
    let _statement = conn.execute(&commandstring).unwrap();

    Ok(())
}

pub fn listrecords(conn: &Connection) {
    let mut statement = conn
        .prepare("SELECT * FROM findings")
        .unwrap();

        while let State::Row = statement.next().unwrap() {
            println!("-----------------------------");
            println!("Title = {}", statement.read::<String>(1).unwrap());
            println!("Finding = {}", statement.read::<String>(2).unwrap());
            println!("Details = {}", statement.read::<String>(3).unwrap());
            println!("Justification = {}", statement.read::<String>(4).unwrap());

        }

}
