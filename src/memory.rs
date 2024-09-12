use std::alloc::{alloc, realloc, Layout};

pub fn grow_capacity(capacity: usize) -> usize {
    if capacity < 8 {
        8
    } else {
        capacity * 2
    }
}

pub fn reallocate<T>(pointer: *mut T, old_capacity: usize, new_capacity: usize) -> *mut T {
    (unsafe {
        // let old_pointer = pointer as *mut u8;
        alloc(Layout::array::<T>(new_capacity).unwrap())
    }) as *mut T
}
