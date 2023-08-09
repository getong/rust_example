use std::cell::RefCell;
use std::thread_local;

struct Foo(*const i32); // a non-Send/Sync type

struct GlobalState {
    foo: Foo,
    data: String,
    mutable_data: RefCell<String>,
}

thread_local! {
    static STATE: GlobalState = GlobalState {
        foo: Foo(std::ptr::null()),
        data: "bla".to_string(),
        mutable_data: RefCell::new("".to_string()),
    };
}

fn main() {
    STATE.with(|state| {
        assert_eq!(state.foo.0, std::ptr::null());
        assert_eq!(state.data, "bla");
        assert_eq!(state.mutable_data.borrow().as_str(), "");
        state.mutable_data.borrow_mut().push_str("xyzzy");
    });
    STATE.with(|state| {
        assert_eq!(state.mutable_data.borrow().as_str(), "xyzzy");
    });
}
