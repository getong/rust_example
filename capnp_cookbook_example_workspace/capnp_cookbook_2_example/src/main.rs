// Note that we use `capnp` here, NOT `capnpc`
extern crate capnp;

// We create a module here to define how we are to access the code
// being included.
pub mod point_capnp {
    // The environment variable OUT_DIR is set by Cargo, and
    // is the location of all the code that was built as part
    // of the codegen step.
    // point_capnp.rs is the actual file to include
    include!(concat!(env!("OUT_DIR"), "/point_capnp.rs"));
}

fn main() {
    // The process of building a Cap'N Proto message is a bit tedious.
    // We start by creating a generic Builder; it acts as the message
    // container that we'll later be filling with content of our `Point`
    let mut builder = capnp::message::Builder::new_default();

    // Because we need a mutable reference to the `builder` later,
    // we fence off this part of the code to allow sequential mutable
    // borrows. As I understand it, non-lexical lifetimes:
    // https://github.com/rust-lang/rust-roadmap/issues/16
    // will make this no longer necessary
    {
        // And now we can set up the actual message we're trying to create
        let mut point_msg = builder.init_root::<point_capnp::point::Builder>();

        // Stuff our message with some content
        point_msg.set_x(12);

        point_msg.set_y(14);
    }

    // It's now time to serialize our message to binary. Let's set up a buffer for that:
    let mut buffer = Vec::new();

    // And actually fill that buffer with our data
    capnp::serialize::write_message(&mut buffer, &builder).unwrap();

    // Finally, let's deserialize the data
    let deserialized = capnp::serialize::read_message(
        &mut buffer.as_slice(),
        capnp::message::ReaderOptions::new(),
    )
    .unwrap();

    // `deserialized` is currently a generic reader; it understands
    // the content of the message we gave it (i.e. that there are two
    // int32 values) but doesn't really know what they represent (the Point).
    // This is where we map the generic data back into our schema.
    let point_reader = deserialized
        .get_root::<point_capnp::point::Reader>()
        .unwrap();

    // We can now get our x and y values back, and make sure they match
    assert_eq!(point_reader.get_x(), 12);
    assert_eq!(point_reader.get_y(), 14);

    let deserialized = capnp::serialize::read_message(
        &mut buffer.as_slice(),
        capnp::message::ReaderOptions::new(),
    )
    .unwrap();

    let point_reader: capnp::message::TypedReader<
        capnp::serialize::OwnedSegments,
        point_capnp::point::Owned,
    > = capnp::message::TypedReader::new(deserialized);

    // Because the point_reader is now working with OwnedSegments (which are owned vectors) and an Owned message
    // (which is 'static lifetime), this is now safe
    let handle = std::thread::spawn(move || {
        // The point_reader owns its data, and we use .get() to retrieve the actual point_capnp::point::Reader
        // object from it
        let point_root = point_reader.get().unwrap();

        assert_eq!(point_root.get_x(), 12);

        assert_eq!(point_root.get_y(), 14);
    });

    handle.join().unwrap();
}
