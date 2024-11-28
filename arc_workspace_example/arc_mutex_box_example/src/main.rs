use std::{
  ops::{Deref, DerefMut},
  sync::{Arc, LockResult, Mutex, MutexGuard, PoisonError},
};

pub type OnTheFlyInner<T> = Box<T>;

#[derive(Clone)]
pub struct OnTheFly<T> {
  inner: Arc<Mutex<Option<OnTheFlyInner<T>>>>,
}

pub struct MutexGuardRef<'a, T> {
  mutex_guard: MutexGuard<'a, Option<Box<T>>>,
}

impl<'a, T> MutexGuardRef<'a, T> {
  pub fn inner(&self) -> Option<&T> {
    match self.mutex_guard.deref() {
      Some(b) => Some(&*b),
      None => None,
    }
  }

  pub fn inner_mut(&mut self) -> Option<&mut T> {
    match self.mutex_guard.deref_mut() {
      Some(b) => Some(&mut *b),
      None => None,
    }
  }
}

impl<T> OnTheFly<T>
where
  T: Sized + Send + Sync,
{
  pub fn new(b: Box<T>) -> OnTheFly<T> {
    OnTheFly {
      inner: Arc::new(Mutex::new(Some(b))),
    }
  }

  pub fn new_empty() -> OnTheFly<T> {
    OnTheFly {
      inner: Arc::new(Mutex::new(None)),
    }
  }

  pub fn replace(&mut self, replace_by: Option<OnTheFlyInner<T>>) {
    if let Some(on_the_fly_inner) = replace_by {
      self.inner.lock().unwrap().replace(on_the_fly_inner);
    } else {
      self.inner.lock().unwrap().take();
    }
  }

  pub fn lock(&self) -> LockResult<MutexGuardRef<'_, T>> {
    match self.inner.lock() {
      Ok(mut m) => match m.as_deref_mut() {
        _ => Ok(MutexGuardRef { mutex_guard: m }),
      },
      Err(e) => Err(PoisonError::new(MutexGuardRef {
        mutex_guard: e.into_inner(),
      })),
    }
  }
}

fn main() {
  let mut o = OnTheFly::new(Box::new(0));
  let oo = o.clone();
  std::thread::spawn(move || loop {
    if let Some(oo) = oo.lock().unwrap().inner_mut() {
      *oo += 1;
      println!("value: {}", *oo);
    }
    std::thread::sleep(std::time::Duration::from_secs(1));
  });
  // Waits a little before substituting the inner object on-the-fly
  std::thread::sleep(std::time::Duration::from_secs(5));
  o.replace(Some(Box::new(12345)));
  std::thread::sleep(std::time::Duration::from_secs(100))
}
