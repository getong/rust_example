use mysql::*;

fn main() {
    let connstr = "mysql://root:Password@192.168.4.25:3306/database";
    let sqlpool = Pool::new(connstr).unwrap();
    let connection = sqlpool.get_conn().unwrap();
    println!("{}", connection.connection_id());
}
