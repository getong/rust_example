use crossbeam_channel::{bounded, select};

#[tokio::main]
async fn main() {
  let (_s, r) = bounded::<usize>(1);

  tokio::spawn(async move {
    let mut counter = 0;
    loop {
      let loop_id = counter.clone();
      tokio::spawn(async move {
        // why this one was not fired?
        println!("inner task {}", loop_id);
      }); // .await.unwrap(); - solves issue, but this is long task which cannot be awaited
      println!("loop {}", loop_id);
      select! {
          recv(r) -> _rr => {
              // match rr {
              //     Ok(ee) => {
              //         println!("received from channel {}", loop_id);
              //         tokio::spawn(async move {
              //             println!("received from channel task {}", loop_id);
              //         });
              //     },
              //     Err(e) => println!("{}", e),
              // };
          },
          // more recv(some_channel) ->
      }
      counter = counter + 1;
    }
  });

  // let s_clone = s.clone();
  // tokio::spawn(async move {
  //     s_clone.send(2).unwrap();
  // });

  loop {
    // rest of the program
  }
}
