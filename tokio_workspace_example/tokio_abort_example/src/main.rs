use tokio::time;

#[tokio::main]
async fn main() {
   let mut handles = Vec::new();

   handles.push(tokio::spawn(async {
      time::sleep(time::Duration::from_secs(10)).await;
      true
   }));

   handles.push(tokio::spawn(async {
      time::sleep(time::Duration::from_secs(10)).await;
      false
   }));

   for handle in &handles {
       handle.abort();
   }

   for handle in handles {
       assert!(handle.await.unwrap_err().is_cancelled());
   }
}
