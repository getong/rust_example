use agones::beta::Beta;

pub async fn counters_and_lists(mut beta: Beta) -> Result<(), String> {
  const COUNTER: &str = "demo-rooms";
  const LIST: &str = "demo-players";
  const PLAYER: &str = "example-player";

  let count = beta
    .get_counter_count(COUNTER)
    .await
    .map_err(|error| format!("get_counter_count({COUNTER}): {error}"))?;
  let counter_capacity = beta
    .get_counter_capacity(COUNTER)
    .await
    .map_err(|error| format!("get_counter_capacity({COUNTER}): {error}"))?;
  let values = beta
    .get_list_values(LIST)
    .await
    .map_err(|error| format!("get_list_values({LIST}): {error}"))?;
  let list_capacity = beta
    .get_list_capacity(LIST)
    .await
    .map_err(|error| format!("get_list_capacity({LIST}): {error}"))?;

  if count != 0 || counter_capacity != 10 || !values.is_empty() || list_capacity != 10 {
    return Err(
      "demo requires demo-rooms count=0/capacity=10 and demo-players values=[]/capacity=10"
        .to_owned(),
    );
  }

  println!("Counter {COUNTER}: count={count}, capacity={counter_capacity}");
  beta
    .set_counter_capacity(COUNTER, 20)
    .await
    .map_err(|error| format!("set_counter_capacity({COUNTER}): {error}"))?;
  beta
    .set_counter_count(COUNTER, 1)
    .await
    .map_err(|error| format!("set_counter_count({COUNTER}): {error}"))?;
  beta
    .increment_counter(COUNTER, 1)
    .await
    .map_err(|error| format!("increment_counter({COUNTER}): {error}"))?;
  let count = beta
    .get_counter_count(COUNTER)
    .await
    .map_err(|error| format!("get_counter_count({COUNTER}): {error}"))?;
  println!("Counter {COUNTER} after set and increment: {count}");
  beta
    .decrement_counter(COUNTER, 1)
    .await
    .map_err(|error| format!("decrement_counter({COUNTER}): {error}"))?;
  beta
    .set_counter_count(COUNTER, 0)
    .await
    .map_err(|error| format!("set_counter_count({COUNTER}): {error}"))?;
  beta
    .set_counter_capacity(COUNTER, counter_capacity)
    .await
    .map_err(|error| format!("set_counter_capacity({COUNTER}): {error}"))?;

  println!("List {LIST}: values={values:?}, capacity={list_capacity}");
  beta
    .set_list_capacity(LIST, 20)
    .await
    .map_err(|error| format!("set_list_capacity({LIST}): {error}"))?;
  beta
    .append_list_value(LIST, PLAYER)
    .await
    .map_err(|error| format!("append_list_value({LIST}): {error}"))?;
  let contains = beta
    .list_contains(LIST, PLAYER)
    .await
    .map_err(|error| format!("list_contains({LIST}): {error}"))?;
  let length = beta
    .get_list_length(LIST)
    .await
    .map_err(|error| format!("get_list_length({LIST}): {error}"))?;
  let values = beta
    .get_list_values(LIST)
    .await
    .map_err(|error| format!("get_list_values({LIST}): {error}"))?;
  println!("List {LIST}: contains({PLAYER})={contains}, length={length}, values={values:?}");
  beta
    .delete_list_value(LIST, PLAYER)
    .await
    .map_err(|error| format!("delete_list_value({LIST}): {error}"))?;
  beta
    .set_list_capacity(LIST, list_capacity)
    .await
    .map_err(|error| format!("set_list_capacity({LIST}): {error}"))?;

  println!("Counters and Lists demo finished; initial values restored");
  Ok(())
}
