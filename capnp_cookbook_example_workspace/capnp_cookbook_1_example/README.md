# capnp_cookbook example

``` shell
brew install capnp
capnp id

cargo new capnp_cookbook_example
cd capnp_cookbook_example
cargo add capnpc --build
cargo add capnp


mkdir schema
cd schema

cd ..
cargo build
find . -name point_capnp.rs
```

copy from [Captain's Cookbook - Part 1](https://bspeice.github.io/captains-cookbook-part-1.html)
