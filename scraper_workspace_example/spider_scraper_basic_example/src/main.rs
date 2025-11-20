use scraper::{Html, Selector};

fn main() {
  let html = r#"
    <ul>
        <li>Foo</li>
        <li>Bar</li>
        <li>Baz</li>
    </ul>
"#;

  let fragment = Html::parse_fragment(html);
  let selector = Selector::parse("li").unwrap();

  for element in fragment.select(&selector) {
    assert_eq!("li", element.value().name());
    for (k, v) in element.value().attrs() {
      println!("k is {:?}, v is {:?}", k, v);
    }
  }
}
