use std::collections::HashMap;
use once_cell::sync::Lazy;
use prost::Name;
mod mypackage {
    include!("mypackage.rs");
}

const MYPACKAGE_MYMESSAGE: i32 = 1;
const MYPACKAGE_OTHERMESSAGE: i32 = 2;

pub static MESSAGE_TO_NUM_LIST: Lazy<HashMap<String, i32>> = Lazy::new(||{
    let mut map = HashMap::new();
    map.insert(mypackage::MyMessage::full_name(), MYPACKAGE_MYMESSAGE);
    map.insert(mypackage::OtherMessage::full_name(), MYPACKAGE_OTHERMESSAGE);
    map
});
