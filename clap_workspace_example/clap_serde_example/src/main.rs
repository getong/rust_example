fn main() {
    // println!("Hello, world!");
    const CLAP_YAML: &'static str = r#"
name: app_clap_serde
version : "1.0"
about : yaml_support!
author : yaml_supporter

args:
    - apple :
        short: a
    - banana:
        short: b
        long: banana
        aliases :
            - musa_spp

subcommands:
    - sub1:
        about : subcommand_1
    - sub2:
        about : subcommand_2

"#;
    let app: clap_serde::CommandWrap = serde_yaml::from_str(CLAP_YAML).expect("fail to make yaml");
    assert_eq!(app.get_name(), "app_clap_serde");
    assert_eq!(app.get_version().unwrap(), "1.0");
}
