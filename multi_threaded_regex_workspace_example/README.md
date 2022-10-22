# multi-threaded regex matching

copy from [Contention on multi-threaded regex matching](https://morestina.net/blog/1827/multi-threaded-regex)


``` rust
for i in basic_regex_example rayon_regex_example thread_rayon_regex_example
do
 cargo run --bin $i
done
```
