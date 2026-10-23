//! `dyn Trait` 的可运行示例。
//!
//! 运行：`cargo run`

use std::{any::Any, rc::Rc, sync::Arc};

/// 所有实现都必须能被当作 `Any`，这样才可以在运行时尝试恢复具体类型。
trait Pet: Any {
  fn name(&self) -> &str;
  fn sound(&self) -> &'static str;

  fn as_any(&self) -> &dyn Any;
  fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// `Animal: Pet` 表示 `Animal` 是 `Pet` 的子 trait，
/// 因此可以从 `dyn Animal` 视图向 `dyn Pet` 视图上转型。
trait Animal: Pet {
  fn legs(&self) -> u8;
}

#[derive(Debug)]
struct Dog {
  name: String,
  age: u8,
}

impl Pet for Dog {
  fn name(&self) -> &str {
    &self.name
  }

  fn sound(&self) -> &'static str {
    "汪"
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}

impl Animal for Dog {
  fn legs(&self) -> u8 {
    4
  }
}

#[derive(Debug)]
struct Cat {
  name: String,
}

impl Pet for Cat {
  fn name(&self) -> &str {
    &self.name
  }

  fn sound(&self) -> &'static str {
    "喵"
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}

/// 从 `&dyn Animal` 取出 `Pet` 视图：这是超 trait upcasting，不是 downcast。
fn upcast_to_pet(animal: &dyn Animal) -> &dyn Pet {
  animal
}

/// 一个与 `Pet` 无继承关系的 trait。
trait Named {
  fn display_name(&self) -> String;
}

/// 无关 trait object 不能直接 `as` 转换，通常用适配器重新提供目标 trait。
struct NamedAdapter<'a> {
  pet: &'a dyn Pet,
}

impl Named for NamedAdapter<'_> {
  fn display_name(&self) -> String {
    format!("宠物: {}", self.pet.name())
  }
}

fn read_by_borrowed_object(pet: &dyn Pet) -> String {
  format!("{} {}", pet.name(), pet.sound())
}

fn read_by_owned_object(pet: Box<dyn Pet>) -> String {
  read_by_borrowed_object(pet.as_ref())
}

fn read_by_shared_object(pet: Arc<dyn Pet + Send + Sync>) -> String {
  read_by_borrowed_object(pet.as_ref())
}

fn demonstrate_recovering_data() {
  let mut pet: Box<dyn Pet> = Box::new(Dog {
    name: "旺财".to_owned(),
    age: 3,
  });

  // trait object 不能直接访问 `Dog::age`；必须通过 trait 方法，或显式 downcast。
  let dog = pet
    .as_any()
    .downcast_ref::<Dog>()
    .expect("pet should contain a Dog");
  assert_eq!(dog.age, 3);

  let dog = pet
    .as_any_mut()
    .downcast_mut::<Dog>()
    .expect("pet should contain a Dog");
  dog.age += 1;
  assert_eq!(dog.age, 4);

  let cat = Cat {
    name: "咪咪".to_owned(),
  };
  assert!(cat.as_any().downcast_ref::<Dog>().is_none());
}

fn demonstrate_object_views_and_containers() {
  let dog = Dog {
    name: "旺财".to_owned(),
    age: 3,
  };

  let borrowed: &dyn Animal = &dog;
  assert_eq!(borrowed.legs(), 4);

  let pet: &dyn Pet = upcast_to_pet(borrowed);
  assert_eq!(read_by_borrowed_object(pet), "旺财 汪");

  let mut mutable_dog = Dog {
    name: "可改名".to_owned(),
    age: 1,
  };
  let mutable_pet: &mut dyn Pet = &mut mutable_dog;
  mutable_pet
    .as_any_mut()
    .downcast_mut::<Dog>()
    .expect("mutable pet should contain a Dog")
    .age += 1;
  assert_eq!(mutable_dog.age, 2);

  let owned: Box<dyn Pet> = Box::new(Dog {
    name: "小黑".to_owned(),
    age: 2,
  });
  assert_eq!(read_by_owned_object(owned), "小黑 汪");

  let shared: Rc<dyn Pet> = Rc::new(Cat {
    name: "咪咪".to_owned(),
  });
  assert_eq!(read_by_borrowed_object(shared.as_ref()), "咪咪 喵");

  let thread_shared: Arc<dyn Pet + Send + Sync> = Arc::new(Dog {
    name: "大黄".to_owned(),
    age: 5,
  });
  assert_eq!(read_by_shared_object(thread_shared), "大黄 汪");
}

fn demonstrate_unrelated_trait_conversion() {
  let dog = Dog {
    name: "旺财".to_owned(),
    age: 3,
  };
  let pet: &dyn Pet = &dog;
  let named = NamedAdapter { pet };
  assert_eq!(named.display_name(), "宠物: 旺财");
}

fn main() {
  demonstrate_recovering_data();
  demonstrate_object_views_and_containers();
  demonstrate_unrelated_trait_conversion();
  println!("trait object examples passed");
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn can_downcast_and_mutate_the_concrete_value() {
    let mut pet: Box<dyn Pet> = Box::new(Dog {
      name: "旺财".to_owned(),
      age: 3,
    });

    let dog = pet.as_any_mut().downcast_mut::<Dog>();
    assert_eq!(dog.map(|dog| dog.age), Some(3));
  }

  #[test]
  fn can_upcast_to_a_supertrait_view() {
    let dog = Dog {
      name: "旺财".to_owned(),
      age: 3,
    };
    let animal: &dyn Animal = &dog;
    let pet: &dyn Pet = animal;
    assert_eq!(pet.name(), "旺财");
  }
}
