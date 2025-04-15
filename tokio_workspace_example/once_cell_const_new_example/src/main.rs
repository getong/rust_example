use primitive_types::U256;
use tokio::sync::{OnceCell, RwLock};

// Define static variables NUM and NUM2
pub static NUM: OnceCell<RwLock<U256>> = OnceCell::const_new();
pub static NUM2: OnceCell<RwLock<U256>> = OnceCell::const_new();
pub static NUM3: OnceCell<RwLock<U256>> = OnceCell::const_new();

#[tokio::main]
async fn main() {
  {
    if let Some(num_lock) = NUM.get() {
      let num = *num_lock.read().await;
      println!("uninitial, the num is {}", num);
    } else {
      println!("uninitial, cannot read num");
    }
  }

  // Initialize the NUM variable using set()
  NUM
    .set(RwLock::new(U256::from(42)))
    .expect("Failed to initialize NUM");

  {
    if let Some(num_lock) = NUM.get() {
      let num = *num_lock.read().await;
      println!("after initial, the num is {}", num);
    } else {
      println!("after initial, cannot read num");
    }
  }

  let num_read = NUM.get().expect("NUM is not initialized").read().await; // Await the read lock
  println!("The value of NUM is: {}", *num_read);

  // Initialize the NUM2 variable directly
  let _ = NUM2.get_or_init(async || RwLock::new(U256::from(84))).await;

  // Read from NUM2
  {
    let num2_read = NUM2.get().expect("NUM2 is not initialized").read().await; // Await the read lock
    println!("The value of NUM2 is: {}", *num2_read);
  }

  // Write to NUM2
  {
    let mut num2_write = NUM2.get().expect("NUM2 is not initialized").write().await; // Await the write lock
    *num2_write = U256::from(200);
    println!("The value of NUM2 has been updated to: {}", *num2_write);
  }

  // Verify the updated value of NUM2
  {
    let num2_read = NUM2.get().expect("NUM2 is not initialized").read().await; // Await the read lock
    println!("The updated value of NUM2 is: {}", *num2_read);
  }

  // this code will panic,
  {
    let mut num3_write = NUM3.get().expect("NUM3 not initialized").write().await;
    *num3_write = U256::from(555);
    println!("NUM3 updated to: {}", *num3_write);
  }

  {
    let num3_read = NUM3.get().expect("NUM3 not initialized").read().await;
    println!("Final value of NUM3: {}", *num3_read);
  }
}
