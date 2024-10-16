use crate::types::Hashable;
use std::ptr::NonNull;
use std::{fmt::Write as _, mem};

use tracing::debug;

use crate::types::string::LoxString;
use crate::types::value::Value;

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

    pub fn insert(&mut self, key: Value, value: Value) -> bool {
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

    pub fn get(&self, key: Value) -> Option<Value> {
        if self.count == 0 {
            return None;
        }

        let entry = Self::find_entry(self.entries, self.capacity, key);
        if unsafe { (*entry).key.is_some() } {
            Some(unsafe { (*entry).value })
        } else {
            None
        }
    }

    fn find_entry(entries: NonNull<Entry>, capacity: u32, key: Value) -> *mut Entry {
        let mut index: u32 = key.hash().0 % capacity;
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
        let new_pointer: NonNull<Entry> = memory::alloc_array(new_capacity as usize);
        for i in 0..new_capacity {
            unsafe {
                *new_pointer.as_ptr().add(i as usize) = Entry {
                    key: None,
                    value: Value::Nil,
                }
            }
        }

        let old_entities = self.entries;
        let old_capacity = self.capacity;

        self.capacity = new_capacity;
        self.entries = new_pointer;
        self.count = 0;

        if old_capacity > 0 {
            for i in 0..old_capacity {
                unsafe {
                    let entry = old_entities.as_ptr().add(i as usize);
                    if let Some(key) = (*entry).key {
                        self.insert(key, (*entry).value);
                    }
                }
            }
            memory::free_array(old_entities, old_capacity as usize);
        }
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
        let mut real_count = 0;
        for i in 0..self.capacity {
            let entry = unsafe { (*self.entries.as_ptr().add(i as usize)).clone() };
            if let Some(key) = entry.key {
                write!(&mut entries, "[{}:{:?}=>{:?}] ", i, key, entry.value)?;
                real_count += 1;
            }
        }
        f.debug_struct("HashTable")
            .field("count", &self.count)
            .field("real_count", &format!("{}", real_count))
            .field("capacity", &self.capacity)
            .field("entries", &entries)
            .finish()
    }
}

#[derive(Debug, Clone)]
struct Entry {
    key: Option<Value>,
    value: Value,
}

#[cfg(test)]
mod tests {

    use crate::{gc::Gc, types::obj::Obj};

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
        let key = Value::Boolean(true);
        let unfound = Value::Nil;
        table.insert(key, Value::Boolean(true));
        assert_eq!(table.capacity, 8);
        assert_eq!(table.count, 1);
        let ret = table.get(key);
        assert_eq!(ret, Some(Value::Boolean(true)));
        assert_eq!(table.get(unfound), None);
    }

    #[test]
    fn insert_two() {
        let mut table: HashTable = HashTable::new();
        let key1 = Value::Boolean(true);
        let key2 = Value::Boolean(false);
        table.insert(key1, Value::Boolean(true));
        table.insert(key2, Value::Boolean(false));
        assert_eq!(table.capacity, 8);
        assert_eq!(table.count, 2);
        let ret = table.get(key1);
        assert_eq!(ret, Some(Value::Boolean(true)));
        let ret = table.get(key2);
        assert_eq!(ret, Some(Value::Boolean(false)));
    }

    #[test]
    fn insert_2049() {
        let mut gc = Gc::new();
        let mut table: HashTable = HashTable::new();
        for i in 0..2049 {
            let obj_ref = gc.manage_string(LoxString::string(format!("key{}", i)));
            let key = Value::Obj(obj_ref);
            table.insert(key, Value::Number(f64::from(i)));
            let ret = table.get(key);
            assert_eq!(ret, Some(Value::Number(f64::from(i))));
        }
        assert_eq!(table.count, 2049);
        assert_eq!(table.capacity, 4096);
    }
}
