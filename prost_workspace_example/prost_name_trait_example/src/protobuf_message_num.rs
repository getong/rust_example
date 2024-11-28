use std::{collections::HashMap, error::Error};

use once_cell::sync::Lazy;
use prost::{Message, Name};

mod mypackage {
  include!("mypackage.rs");
}

const MYPACKAGE_MYMESSAGE: i32 = 1;
const MYPACKAGE_OTHERMESSAGE: i32 = 2;

pub static MESSAGE_TO_NUM_LIST: Lazy<HashMap<String, i32>> = Lazy::new(|| {
  let mut map = HashMap::new();
  map.insert(mypackage::MyMessage::full_name(), MYPACKAGE_MYMESSAGE);
  map.insert(mypackage::OtherMessage::full_name(), MYPACKAGE_OTHERMESSAGE);
  map
});

pub fn decode_by_num(num: i32, bytes: &[u8]) -> Result<Box<dyn Message>, Box<dyn Error>> {
  match num {
    MYPACKAGE_MYMESSAGE => Ok(Box::new(mypackage::MyMessage::decode(bytes)?)),
    MYPACKAGE_OTHERMESSAGE => Ok(Box::new(mypackage::OtherMessage::decode(bytes)?)),
    _ => Err(Box::new(std::io::Error::from(std::io::ErrorKind::NotFound))),
  }
}
