mod types;

use std::{collections::BTreeSet, rc::Rc};

use chrono::NaiveDate;
use types::*;
use uuid::Uuid;

fn main() {
  // Basic user
  let user = User {
    user_id: 1,
    first_name: "Alice".into(),
    last_name: "Smith".into(),
    role: Role::Admin,
    family: vec![],
    gender: Gender::Female,
    token: Uuid::new_v4(),
    created_at: NaiveDate::from_ymd_opt(2023, 1, 1)
      .unwrap()
      .and_hms_opt(0, 0, 0)
      .unwrap(),
  };

  // All Vehicle variants
  let bike = Vehicle::Bicycle {
    color: "Red".into(),
  };
  let car = Vehicle::Car {
    brand: "Toyota".into(),
    color: "Blue".into(),
  };

  // All SimpleEnum variants
  let se_a = SimpleEnum::A;
  let se_b = SimpleEnum::B;

  // Series for V variant
  let series = Series {
    points: vec![Point {
      time: 1,
      value: 100,
    }],
  };

  // All ComplexEnum variants
  let ce_a = ComplexEnum::A;
  let ce_b = ComplexEnum::B {
    foo: "Foo".into(),
    bar: 1.23,
  };
  let ce_w = ComplexEnum::W(SimpleEnum::A);
  let ce_f = ComplexEnum::F {
    nested: SimpleEnum::B,
  };
  let ce_v = ComplexEnum::V(vec![series.clone()]);
  let ce_u = ComplexEnum::U(Box::new(user.clone()));

  // All InlineComplexEnum variants
  let ice_a = InlineComplexEnum::A;
  let ice_b = InlineComplexEnum::B {
    foo: "Bar".into(),
    bar: 9.99,
  };
  let ice_w = InlineComplexEnum::W(SimpleEnum::A);
  let ice_f = InlineComplexEnum::F {
    nested: SimpleEnum::B,
  };
  let ice_v = InlineComplexEnum::V(vec![series.clone()]);
  let ice_u = InlineComplexEnum::U(Box::new(user.clone()));

  // ComplexStruct using Rc<BTreeSet>
  let mut tree = BTreeSet::new();
  tree.insert("alpha".into());
  tree.insert("beta".into());

  let complex_struct = ComplexStruct {
    string_tree: Some(Rc::new(tree)),
  };

  // Print everything to demonstrate serialization
  println!("User:\n{}", serde_json::to_string_pretty(&user).unwrap());
  println!("Bike:\n{}", serde_json::to_string_pretty(&bike).unwrap());
  println!("Car:\n{}", serde_json::to_string_pretty(&car).unwrap());

  println!(
    "SimpleEnum A:\n{}",
    serde_json::to_string_pretty(&se_a).unwrap()
  );
  println!(
    "SimpleEnum B:\n{}",
    serde_json::to_string_pretty(&se_b).unwrap()
  );

  println!(
    "ComplexEnum A:\n{}",
    serde_json::to_string_pretty(&ce_a).unwrap()
  );
  println!(
    "ComplexEnum B:\n{}",
    serde_json::to_string_pretty(&ce_b).unwrap()
  );
  println!(
    "ComplexEnum W:\n{}",
    serde_json::to_string_pretty(&ce_w).unwrap()
  );
  println!(
    "ComplexEnum F:\n{}",
    serde_json::to_string_pretty(&ce_f).unwrap()
  );
  println!(
    "ComplexEnum V:\n{}",
    serde_json::to_string_pretty(&ce_v).unwrap()
  );
  println!(
    "ComplexEnum U:\n{}",
    serde_json::to_string_pretty(&ce_u).unwrap()
  );

  println!(
    "InlineComplexEnum A:\n{}",
    serde_json::to_string_pretty(&ice_a).unwrap()
  );
  println!(
    "InlineComplexEnum B:\n{}",
    serde_json::to_string_pretty(&ice_b).unwrap()
  );
  println!(
    "InlineComplexEnum W:\n{}",
    serde_json::to_string_pretty(&ice_w).unwrap()
  );
  println!(
    "InlineComplexEnum F:\n{}",
    serde_json::to_string_pretty(&ice_f).unwrap()
  );
  println!(
    "InlineComplexEnum V:\n{}",
    serde_json::to_string_pretty(&ice_v).unwrap()
  );
  println!(
    "InlineComplexEnum U:\n{}",
    serde_json::to_string_pretty(&ice_u).unwrap()
  );

  println!(
    "Series:\n{}",
    serde_json::to_string_pretty(&series).unwrap()
  );
  println!(
    "ComplexStruct:\n{}",
    serde_json::to_string_pretty(&complex_struct).unwrap()
  );

  // Use Role::User to avoid dead_code warning
  let role_user = Role::User;
  println!(
    "Role::User: {}",
    serde_json::to_string_pretty(&role_user).unwrap()
  );

  // Use Gender::Male and Gender::Other to avoid warnings
  let gender_male = Gender::Male;
  let gender_other = Gender::Other;
  println!(
    "Gender::Male: {}",
    serde_json::to_string_pretty(&gender_male).unwrap()
  );
  println!(
    "Gender::Other: {}",
    serde_json::to_string_pretty(&gender_other).unwrap()
  );
}
