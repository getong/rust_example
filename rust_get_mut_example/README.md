# rust get_mut example

``` rust

pub fn get_mut<I>(
    &mut self,
    index: I
) -> Option<&mut <I as SliceIndex<[T]>>::Output>
where
    I: SliceIndex<[T]>,
```

Returns a mutable reference to an element or subslice depending on the type of index (see get) or None if the index is out of bounds.
