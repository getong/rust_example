use std::mem::MaybeUninit;
use std::ptr::addr_of_mut;

struct Role {
    name: &'static str,
    disabled: bool,
    flag: u32,
}

fn main() {
    let role = unsafe {
        let mut uninit = MaybeUninit::<Role>::uninit();
        let role = uninit.as_mut_ptr();

        addr_of_mut!((*role).name).write_unaligned("basic");
        addr_of_mut!((*role).flag).write_unaligned(1);
        addr_of_mut!((*role).disabled).write_unaligned(false);

        uninit.assume_init()
    };

    println!("{} ({}, {})", role.name, role.flag, role.disabled);
}
