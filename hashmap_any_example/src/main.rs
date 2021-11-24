use std::any::Any;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Default)]
struct AnyMap<K>(HashMap<K, Box<dyn Any>>);

#[derive(Debug)]
enum GetError {
    EmptyKey,
    MismatchedType,
}

impl<K: Hash + Eq> AnyMap<K> {
    fn insert<T: Any>(&mut self, key: K, value: T) {
        self.0.insert(key, Box::new(value));
    }

    fn get<T: Any, Q: ?Sized + Hash + Eq>(&self, key: &Q) -> Option<&T>
    where
        K: Borrow<Q>,
    {
        self.0.get(key)?.downcast_ref()
    }

    fn get_result<T: Any, Q: ?Sized + Hash + Eq>(&self, key: &Q) -> Result<&T, GetError>
    where
        K: Borrow<Q>,
    {
        self.0
            .get(key)
            .ok_or(GetError::EmptyKey)?
            .downcast_ref()
            .ok_or(GetError::MismatchedType)
    }
}

fn main() {
    let mut map = AnyMap::<String>::default();
    map.insert("hello".to_string(), "1");
    map.insert("world".to_string(), 1u32);
    println!("get:");
    println!("{:?}", map.get::<&str, _>("hello"));
    println!("{:?}", map.get::<u32, _>("world"));
    println!("{:?}", map.get::<i32, _>("world"));
    println!("{:?}", map.get::<i32, _>("empty"));
    println!();
    println!("get_result:");
    println!("{:?}", map.get_result::<&str, _>("hello"));
    println!("{:?}", map.get_result::<u32, _>("world"));
    println!("{:?}", map.get_result::<i32, _>("world"));
    println!("{:?}", map.get_result::<i32, _>("empty"));
}
