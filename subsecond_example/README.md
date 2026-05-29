# Subsecond Hot-Code Actor Example

This binary demonstrates the application-side shape of an Erlang-style hot-code
loop with `subsecond`, without using `dx`.

The actor starts with `StateV1`. Its long-lived process state is stored outside
the hot function boundary, while each message is handled through `subsecond::call`.
When a patch is observed, `code_changed` can migrate that live state to `StateV2`.

## Run

```sh
cargo run
```

Commands:

- `inc [n]`: add to the counter
- `dec [n]`: subtract from the counter
- `status`: print the current state version and fields
- `upgrade`: manually migrate `StateV1` to `StateV2`
- `patch`: simulate a hot-patch notification and run `code_changed`
- `help`: print commands
- `quit`: stop the actor

Example session:

```text
inc 5
status
upgrade
inc 3
status
quit
```

## State Versions

`StateV1` contains:

- `counter`
- `ticks`
- `code_generation`

`StateV2` keeps those fields and adds:

- `total_delta`
- `last_event`

The upgrade path preserves `counter`, `ticks`, and `code_generation`, then
initializes the new V2-only fields. This is the same place where an Erlang
process would implement `code_change/3`.

## Hot-Patch Boundary

The example intentionally does not use `dx`. `subsecond` is still used at the
runtime boundary:

- `subsecond::call` wraps message handling and `code_changed`.
- `subsecond::register_handler` records that a patch was applied.
- A non-`dx` patcher would compile a patch library, build a `JumpTable`, and call
  `subsecond::apply_patch`.

The `patch` command is only a local driver for the state migration path; it does
not compile or inject new machine code.

## Important Limit

Keep the outer `ProcessState` enum layout stable while the process is running.
Subsecond can patch function bodies, globals, statics, and thread locals, but its
documentation currently warns that changing struct layout at runtime is not a
safe path. Predeclaring `StateV1` and `StateV2` lets this example demonstrate
state migration without changing the live layout after startup.
