use std::{
    alloc::{alloc, dealloc, handle_alloc_error, realloc, Layout},
    ptr::NonNull,
};

pub fn grow_capacity(capacity: usize) -> usize {
    if capacity < 8 {
        8
    } else {
        capacity * 2
    }
}

pub fn reallocate<T>(pointer: NonNull<T>, old_capacity: usize, new_capacity: usize) -> NonNull<T> {
    let old_ptr = pointer.as_ptr() as *mut u8;
    let new_layout = Layout::array::<T>(new_capacity).unwrap();
    assert!(
        new_layout.size() <= isize::MAX as usize,
        "Allocation too large"
    );
    let new_ptr = match (old_capacity, new_capacity) {
        (0, 0) => return pointer,
        (old, 0) => {
            // SAFETY:
            // old_ptr is guaranteed to be allocated by the same allocator
            // layout is the same as when allocating due to the matches below
            unsafe { dealloc(old_ptr, Layout::array::<T>(old).unwrap()) }
            return NonNull::dangling();
        }
        (0, _new) => {
            // SAFETY:
            // layout has guaranteed non-zero size as 0 size new has been matched above
            unsafe { alloc(new_layout) }
        }
        (old, _) => {
            // SAFETY:
            // old_ptr is guaranteed to be allocated by the same allocator
            // layout is the same as when allocating due to the matches below and therefore also not 0 sized
            // new_size does not overflow isize max (assert above)
            unsafe { realloc(old_ptr, Layout::array::<T>(old).unwrap(), new_layout.size()) }
        }
    };

    match NonNull::new(new_ptr as *mut T) {
        Some(p) => p,
        None => handle_alloc_error(new_layout),
    }
}
