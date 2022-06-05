# arc mutex channel example


copy from [Understanding Rust Thread Safety](https://onesignal.com/blog/thread-safety-rust/)

``` rust
impl<T: ?Sized + Send> Send for RwLock<T>
impl<T: ?Sized + Send + Sync> Sync for RwLock<T>

impl<T: ?Sized + Send> Send for Mutex<T>
impl<T: ?Sized + Send> Sync for Mutex<T>

impl<T: Send> Send for Sender<T>
impl<T> !Sync for Sender<T>

impl<T: ?Sized + Sync + Send> Send for Arc<T>
impl<T: ?Sized + Sync + Send> Sync for Arc<T>
```

The !Sync syntax indicates that Sender<T> is explicitly not Sync.

Sender<T> is not Sync. It's not safe to have multiple immutable references to a Sender live across multiple threads at the same time. Considering that Sender::send(&self) requires only an immutable reference, there must be some kind of interior mutability going on.

Mutex<T> is both Send and Sync if T inside of it is Send. If something is inside of a Mutex, it will never have multiple immutable references live at the same time since Mutex does not allow multiple locks (read or write) to be taken at the same time. Because of this, Mutex<T> effectively bypasses the restrictions of Sync.

RwLock<T> is Send where T is Send, but it is only Sync if T is both Send + Sync. Since most types are Send + Sync, this is a non-issue. It only causes problems because the Sender<T> within it is not Sync. Recall from our explanation that RwLock allows multiple read locks to be open in parallel.

Arc<T> is only Send and Sync if the underlying T is both Send + Sync, meaning that you cannot send an Arc<T> across thread boundaries where T: !Sync.
