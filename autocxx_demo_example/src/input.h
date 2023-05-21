#pragma once

#include <cstdint>
#include <sstream>
#include <stdint.h>
#include <string>

class Goat {
public:
  Goat() : horns(0) {}
  void add_a_horn();
  std::string describe() const;
private:
  uint32_t horns;
};

inline uint32_t DoMath(uint32_t a) {
  return a * 3;
}

inline void Goat::add_a_horn() { horns++; }
inline std::string Goat::describe() const {
  std::ostringstream oss;
  std::string plural = horns == 1 ? "" : "s";
  oss << "This goat has " << horns << " horn" << plural << ".";
  return oss.str();
}
