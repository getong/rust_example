use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;

fn main() {
    //               sec  min   hour   day of month   month   day of week   year
    let expression = "0   30   9,12,15     1,30       May-Oct  Mon,Wed,Fri  2022/10";
    let schedule = Schedule::from_str(expression).unwrap();
    println!("Upcoming fire times:");
    for datetime in schedule.upcoming(Utc).take(10) {
        println!("-> {}", datetime);
    }
}
