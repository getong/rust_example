use frunk::hlist;
use frunk::hlist_pat;
use frunk::LabelledGeneric;
use frunk::{self, monoid, Generic, Semigroup};

#[derive(Generic, LabelledGeneric)]
struct ApiUser<'a> {
    first_name: &'a str,
    last_ame: &'a str,
    age: usize,
}

#[derive(Generic, LabelledGeneric)]
struct NewUser<'a> {
    first_name: &'a str,
    last_name: &'a str,
    age: usize,
}

#[derive(LabelledGeneric)]
struct SavedUser<'a> {
    first_name: &'a str,
    last_name: &'a str,
    age: usize,
}

// Uh-oh ! last_name and first_name have been flipped!
#[derive(LabelledGeneric)]
struct DeletedUser<'a> {
    last_name: &'a str,
    first_name: &'a str,
    age: usize,
}

fn main() {
    // println!("Hello, world!");
    // Combining Monoids
    let v = vec![Some(1), Some(3)];
    assert_eq!(monoid::combine_all(&v), Some(4));

    // HLists
    let h = hlist![1, "hi"];
    assert_eq!(h.len(), 2);
    let hlist_pat!(a, b) = h;
    assert_eq!(a, 1);
    assert_eq!(b, "hi");

    let h1 = hlist![Some(1), 3.3, 53i64, "hello".to_owned()];
    let h2 = hlist![Some(2), 1.2, 1i64, " world".to_owned()];
    let h3 = hlist![Some(3), 4.5, 54, "hello world".to_owned()];
    assert_eq!(h1.combine(&h2), h3);

    // Generic and LabelledGeneric-based programming
    // Allows Structs to play well easily with HLists

    // Instantiate a struct from an HList. Note that you can go the other way too.
    let a_user: ApiUser = frunk::from_generic(hlist!["Joe", "Blow", 30]);

    // Convert using Generic
    let n_user: NewUser = Generic::convert_from(a_user); // done

    // Convert using LabelledGeneric
    //
    // This will fail if the fields of the types converted to and from do not
    // have the same names or do not line up properly :)
    //
    // Also note that we're using a helper method to avoid having to use universal
    // function call syntax
    let s_user: SavedUser = frunk::labelled_convert_from(n_user);

    assert_eq!(s_user.first_name, "Joe");
    assert_eq!(s_user.last_name, "Blow");
    assert_eq!(s_user.age, 30);

    // let d_user = <DeletedUser as LabelledGeneric>::convert_from(s_user); <-- this would fail at compile time :)

    // This will, however, work, because we make use of the Sculptor type-class
    // to type-safely reshape the representations to align/match each other.
    let d_user: DeletedUser = frunk::transform_from(s_user);
    assert_eq!(d_user.first_name, "Joe");
}
