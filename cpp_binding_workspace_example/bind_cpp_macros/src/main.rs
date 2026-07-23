use cpp::cpp;

cpp! {{
    #include <iostream>
    #include "toml.hpp"
}}

fn main() {
  let path = c"Cargo.toml";
  let path_ptr = path.as_ptr();

  unsafe {
    cpp!([path_ptr as "const char *"] {
        toml::parse_result result =
            toml::parse_file(path_ptr);
        if (result) {
            std::cout << result << std::endl;
        } else {
            std::cout
                << "Error reading TOML file"
                << std::endl;
        }
    })
  };
}
