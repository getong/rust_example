use handlebars::Handlebars;

pub mod dynamic;
pub mod generic_static;
pub mod struct_static;
pub mod trait_static;

pub fn build_templates() -> Handlebars<'static> {
  let mut handlebars = handlebars::Handlebars::new();
  handlebars
    .register_template_string(
      "index",
      r#"
                <html>
                    <body>
                        <h1>{{ title }}</h1>
                        <ul>
                            {{#each items }}
                                <li><a href="/item/{{ this.uuid }}">{{ this.name }}</a></li>
                            {{/each}}
                        </ul>
                    </body>
                </html>
            "#,
    )
    .expect("Invalid <index> template");

  handlebars
    .register_template_string(
      "show",
      r#"
                <html>
                    <body>
                        <h1>{{ name }}</h1>
                        <p>uuid: {{ uuid }}</p>
                        <a href="/">&larr; back to list</a>
                    </body>
                </html>
            "#,
    )
    .expect("Invalid template");

  handlebars
}
