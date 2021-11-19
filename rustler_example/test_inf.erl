-module (test_inf).

-export([add/2,

add_u32/2 ,
add_i32/2 ,
echo_u8/1 ,
option_inc/1 ,
result_to_int/1 ,
sum_list/1 ,
make_list/0 ,
term_debug/1 ,
term_eq/2 ,
term_cmp/2 ,
sum_map_values/1 ,
map_entries_sorted/1 ,
map_from_arrays/2 ,
map_generic/1 ,
resource_make/0 ,
resource_set_integer_field/2 ,
resource_get_integer_field/1 ,
resource_make_immutable/1 ,
resource_immutable_count/0 ,
make_shorter_subbinary/1 ,
parse_integer/1 ,
binary_new/0 ,
owned_binary_new/0 ,
unowned_to_owned/1 ,
realloc_shrink/0 ,
realloc_grow/0 ,
encode_string/0 ,
decode_iolist/1 ,
atom_to_string/1 ,
atom_equals_ok/1 ,
binary_to_atom/1 ,
binary_to_existing_atom/1 ,
threaded_fac/1 ,
threaded_sleep/1 ,
send_all/2 ,
sublists/1 ,
tuple_echo/1 ,
record_echo/1 ,
map_echo/1 ,
exception_echo/1 ,
struct_echo/1 ,
unit_enum_echo/1 ,
untagged_enum_echo/1 ,
untagged_enum_with_truthy/1 ,
untagged_enum_for_issue_370/1 ,
newtype_echo/1 ,
tuplestruct_echo/1 ,
newtype_record_echo/1 ,
tuplestruct_record_echo/1 ,
reserved_keywords_type_echo/1 ,
dirty_io/0 ,
dirty_cpu/0 ,
sum_range/1 ,
bad_arg_error/0 ,
atom_str_error/0 ,
raise_atom_error/0 ,
raise_term_with_string_error/0 ,
raise_term_with_atom_error/0 ,
term_with_tuple_error/0 ,
nif_attrs_can_rename/0 ]).

-on_load(init/0).


init() ->
    %% ok = erlang:load_nif("target/debug/libtest_inf", none).
    ok = erlang:load_nif("target/debug/librustler_example", none).

add(_X, _Y) ->
    exit(nif_library_not_loaded).



  add_u32(_, _) -> exit(nif_library_not_loaded).
  add_i32(_, _) -> exit(nif_library_not_loaded).
  echo_u8(_) -> exit(nif_library_not_loaded).
  option_inc(_) -> exit(nif_library_not_loaded).
  result_to_int(_) -> exit(nif_library_not_loaded).

  sum_list(_) -> exit(nif_library_not_loaded).
  make_list() -> exit(nif_library_not_loaded).

  term_debug(_) -> exit(nif_library_not_loaded).
  term_eq(_, _) -> exit(nif_library_not_loaded).
  term_cmp(_, _) -> exit(nif_library_not_loaded).

  sum_map_values(_) -> exit(nif_library_not_loaded).
  map_entries_sorted(_) -> exit(nif_library_not_loaded).
  map_from_arrays(_keys, _values) -> exit(nif_library_not_loaded).
  map_generic(_) -> exit(nif_library_not_loaded).

  resource_make() -> exit(nif_library_not_loaded).
  resource_set_integer_field(_, _) -> exit(nif_library_not_loaded).
  resource_get_integer_field(_) -> exit(nif_library_not_loaded).
  resource_make_immutable(_) -> exit(nif_library_not_loaded).
  resource_immutable_count() -> exit(nif_library_not_loaded).

  make_shorter_subbinary(_) -> exit(nif_library_not_loaded).
  parse_integer(_) -> exit(nif_library_not_loaded).
  binary_new() -> exit(nif_library_not_loaded).
  owned_binary_new() -> exit(nif_library_not_loaded).
  unowned_to_owned(_) -> exit(nif_library_not_loaded).
  realloc_shrink() -> exit(nif_library_not_loaded).
  realloc_grow() -> exit(nif_library_not_loaded).
  encode_string() -> exit(nif_library_not_loaded).
  decode_iolist(_) -> exit(nif_library_not_loaded).

  atom_to_string(_) -> exit(nif_library_not_loaded).
  atom_equals_ok(_) -> exit(nif_library_not_loaded).
  binary_to_atom(_) -> exit(nif_library_not_loaded).
  binary_to_existing_atom(_) -> exit(nif_library_not_loaded).

  threaded_fac(_) -> exit(nif_library_not_loaded).
  threaded_sleep(_) -> exit(nif_library_not_loaded).

  send_all(_, _) -> exit(nif_library_not_loaded).
  sublists(_) -> exit(nif_library_not_loaded).

  tuple_echo(_) -> exit(nif_library_not_loaded).
  record_echo(_) -> exit(nif_library_not_loaded).
  map_echo(_) -> exit(nif_library_not_loaded).
  exception_echo(_) -> exit(nif_library_not_loaded).
  struct_echo(_) -> exit(nif_library_not_loaded).
  unit_enum_echo(_) -> exit(nif_library_not_loaded).
  untagged_enum_echo(_) -> exit(nif_library_not_loaded).
  untagged_enum_with_truthy(_) -> exit(nif_library_not_loaded).
  untagged_enum_for_issue_370(_) -> exit(nif_library_not_loaded).
  newtype_echo(_) -> exit(nif_library_not_loaded).
  tuplestruct_echo(_) -> exit(nif_library_not_loaded).
  newtype_record_echo(_) -> exit(nif_library_not_loaded).
  tuplestruct_record_echo(_) -> exit(nif_library_not_loaded).
  reserved_keywords_type_echo(_) -> exit(nif_library_not_loaded).

  dirty_io() -> exit(nif_library_not_loaded).
  dirty_cpu() -> exit(nif_library_not_loaded).

  sum_range(_) -> exit(nif_library_not_loaded).

  bad_arg_error() -> exit(nif_library_not_loaded).
  atom_str_error() -> exit(nif_library_not_loaded).
  raise_atom_error() -> exit(nif_library_not_loaded).
  raise_term_with_string_error() -> exit(nif_library_not_loaded).
  raise_term_with_atom_error() -> exit(nif_library_not_loaded).
  term_with_tuple_error() -> exit(nif_library_not_loaded).

  nif_attrs_can_rename() -> exit(nif_library_not_loaded).