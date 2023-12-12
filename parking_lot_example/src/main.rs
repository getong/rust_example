use parking_lot::{Mutex, RwLock, RwLockUpgradableReadGuard, RwLockWriteGuard};

#[derive(Default, Debug)]
pub struct Payload {
  pub value: u32,
}

#[derive(Default, Debug)]
pub struct Scope {
  data: RwLock<Vec<Mutex<Option<Payload>>>>,
}

impl Scope {
  pub fn set(&self, pos: usize, val: Payload) {
    let data = self.data.upgradable_read();
    if data.len() <= pos {
      // need to resize the table
      let mut wdata = RwLockUpgradableReadGuard::upgrade(data);
      wdata.resize_with(pos + 1, Default::default);
      let data = RwLockWriteGuard::downgrade(wdata);
      *data[pos].lock() = Some(val);
    } else {
      *data[pos].lock() = Some(val);
    }
  }

  pub fn into_data(self) -> Vec<Option<Payload>> {
    self
      .data
      .into_inner()
      .into_iter()
      .map(Mutex::into_inner)
      .collect()
  }

  pub fn into_data_vec(self) -> Vec<Mutex<Option<Payload>>> {
    self.data.into_inner()
  }

  pub fn optimize_set(&self, pos: usize, val: Payload) {
    let mut data = self.data.read();
    if data.len() <= pos {
      // "upgrade" the lock
      drop(data);
      let mut wdata = self.data.write();
      // check that someone else hasn't resized the table in the meantime
      if wdata.len() <= pos {
        wdata.resize_with(pos + 1, Default::default);
      }
      // now "downgrade" it back again
      drop(wdata);
      data = self.data.read();
    }
    *data[pos].lock() = Some(val);
  }
}

fn main() {
  // println!("Hello, world!");
  let scope = Scope::default();

  scope.optimize_set(0, Payload { value: 0 });
  scope.optimize_set(1, Payload { value: 1 });
  println!("scope : {:?}", scope);
}
