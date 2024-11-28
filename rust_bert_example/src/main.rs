// brew install pytorch jq
// export LIBTORCH=$(brew --cellar pytorch)/$(brew info --json pytorch | jq -r
// '.[0].installed[0].version') export LD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH
fn main() {
  let qa_model = QuestionAnsweringModel::new(Default::default())?;

  let question = String::from("Where does Amy live ?");
  let context = String::from("Amy lives in Amsterdam");

  let answers = qa_model.predict(&[QaInput { question, context }], 1, 32);
  println!("answers: {:?}", answers);
}
