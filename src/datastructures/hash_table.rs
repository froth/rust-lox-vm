use std::fmt::Write as _;
use std::ptr::NonNull;

use crate::value::{LoxString, Value};

use super::memory;

const MAX_LOAD: f32 = 0.75;

pub struct HashTable {
    // these are u32 because the hashing function is u32. This is converted to and from usize without checks
    // because we know we are inside of the limits and we don't support 16 bit platforms.
    // TODO: change these and hashing to usize?
    count: u32,
    capacity: u32,
    entries: NonNull<Entry>,
}

impl HashTable {
    pub fn new() -> Self {
        Self {
            count: 0,
            capacity: 0,
            entries: NonNull::dangling(),
        }
    }

    pub fn insert(&mut self, key: *const LoxString, value: Value) -> bool {
        if (self.count + 1) as f32 > MAX_LOAD * self.capacity as f32 {
            let new_capacity: u32 = u32::try_from(memory::grow_capacity(self.capacity as usize))
                .expect("max capacity is u32");
            self.adjust_capacity(new_capacity);
        }

        let entry = Self::find_entry(self.entries, self.capacity, key);
        let is_new_key = unsafe { (*entry).key.is_none() };
        if is_new_key {
            self.count += 1;
        }
        unsafe {
            (*entry).key = Some(key);
            (*entry).value = value;
        }
        is_new_key
    }

    fn find_entry(entries: NonNull<Entry>, capacity: u32, key: *const LoxString) -> *mut Entry {
        let mut index: u32 = unsafe { (*key).hash } % capacity;
        loop {
            // SAFETY: we know this ends in valid memory of HashTable
            let entry = unsafe { entries.as_ptr().add(index as usize) };
            let found_key = unsafe { (*entry).key };
            if found_key.is_none() || found_key.is_some_and(|k| k == key) {
                return entry;
            }
            index = (index.wrapping_add(1)) % capacity
        }
    }

    fn adjust_capacity(&mut self, new_capacity: u32) {
        let new_pointer =
            memory::reallocate(self.entries, self.capacity as usize, new_capacity as usize);
        for i in 0..new_capacity {
            unsafe {
                *new_pointer.as_ptr().add(i as usize) = Entry {
                    key: None,
                    value: Value::Nil,
                }
            }
        }
        self.entries = new_pointer;
        self.capacity = new_capacity;
        // self.count = self.count; // TODO
    }
}

impl Drop for HashTable {
    fn drop(&mut self) {
        if self.capacity != 0 {
            memory::reallocate(self.entries, self.capacity as usize, 0);
        }
    }
}

impl std::fmt::Debug for HashTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut entries = String::new();
        for i in 0..self.capacity {
            let entry = unsafe { (*self.entries.as_ptr().add(i as usize)).clone() };
            if let Some(key) = entry.key {
                let key = unsafe { (*key).clone() };
                write!(&mut entries, "[{:?}=>{:?}] ", key, entry.value)?;
            }
        }
        f.debug_struct("HashTable")
            .field("count", &self.count)
            .field("capacity", &self.capacity)
            .field("entries", &entries)
            .finish()
    }
}

#[derive(Debug, Clone)]
struct Entry {
    key: Option<*const LoxString>,
    value: Value,
}

#[cfg(test)]
mod tests {

    use crate::{gc::Gc, value::Obj};

    use super::*;

    #[test]
    fn new_works() {
        let table: HashTable = HashTable::new();
        assert_eq!(table.capacity, 0);
        assert_eq!(table.count, 0);
    }

    #[test]
    fn insert_one() {
        let mut table: HashTable = HashTable::new();
        let s = LoxString::from_str("key");
        let s1 = LoxString::from_str("key1");
        let value = Obj::from_str("asfsafsafd");
        let mut gc = Gc::new();
        let value = gc.manage(value);
        table.insert(&s, Value::Boolean(true));
        table.insert(&s, Value::Boolean(true));
        table.insert(&s1, Value::Obj(value));
        assert_eq!(table.capacity, 8);
        assert_eq!(table.count, 2);
        println!("{:?}", table);
    }
}
