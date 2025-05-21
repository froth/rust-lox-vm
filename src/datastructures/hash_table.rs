use crate::types::obj::Obj;
use crate::types::obj_ref::ObjRef;
use crate::types::string::hash_str;
use crate::types::Hashable;
use std::fmt::Write as _;
use std::ptr::NonNull;

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
        //SAFETY: we are sure the pointer points into valid HashTable memory
        unsafe {
            let is_new_key = (*entry).key.is_none();
            if is_new_key && !(*entry).is_tombstone() {
                self.count += 1;
            }
            (*entry).key = Some(key);
            (*entry).value = value;
            is_new_key
        }
    }

    pub fn get(&self, key: Value) -> Option<Value> {
        if self.count == 0 {
            return None;
        }

        let entry = Self::find_entry(self.entries, self.capacity, key);
        //SAFETY: we are sure the pointer points into valid HashTable memory
        unsafe {
            if (*entry).key.is_some() {
                Some((*entry).value)
            } else {
                None
            }
        }
    }

    pub fn delete(&mut self, key: Value) -> bool {
        if self.capacity == 0 {
            return false;
        }
        let entry = Self::find_entry(self.entries, self.capacity, key);
        unsafe {
            if (*entry).key.is_none() {
                false
            } else {
                (*entry).make_tombstone();
                true
            }
        }
    }

    pub fn add_all(&mut self, from: &Self) {
        for i in 0..from.capacity {
            unsafe {
                let entry = from.entries.as_ptr().add(i as usize);
                if let Some(key) = (*entry).key {
                    self.insert(key, (*entry).value);
                }
            }
        }
    }

    fn find_entry(entries: NonNull<Entry>, capacity: u32, key: Value) -> *mut Entry {
        let mut index: u32 = key.hash().0 % capacity;
        let mut tombstone = None;
        loop {
            // SAFETY: we know this ends in valid memory of HashTable
            unsafe {
                let entry = entries.as_ptr().add(index as usize);
                if let Some(entry_key) = (*entry).key {
                    // this does pointer equality for all Obj, this only works because all Strings are interned
                    if entry_key == key {
                        return entry;
                    }
                } else if (*entry).is_tombstone() {
                    tombstone.get_or_insert(entry);
                } else {
                    return tombstone.unwrap_or(entry);
                }
            }
            index = (index.wrapping_add(1)) % capacity
        }
    }

    // only for string interning
    pub fn find_string(&self, string: &str) -> Option<ObjRef> {
        if self.count == 0 {
            return None;
        }
        let mut index = hash_str(string).0 % self.capacity;
        loop {
            // SAFETY: we know this ends in valid memory of HashTable
            unsafe {
                let entry = self.entries.as_ptr().add(index as usize);
                if let Some(Value::Obj(obj_ref)) = (*entry).key {
                    let obj = &(*obj_ref);
                    if let Obj::String(s) = obj {
                        if s.string.eq(string) {
                            return Some(obj_ref);
                        }
                    }
                } else if !(*entry).is_tombstone() {
                    return None;
                }
            }
            index = (index.wrapping_add(1)) % self.capacity;
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

impl Entry {
    fn make_tombstone(&mut self) {
        self.key = None;
        self.value = Value::Boolean(true)
    }

    fn is_tombstone(&self) -> bool {
        self.key.is_none() && self.value == Value::Boolean(true)
    }
}

#[cfg(test)]
mod tests {

    use crate::gc::Gc;

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
        let inserted = table.insert(key1, Value::Boolean(true));
        assert!(inserted);
        let inserted = table.insert(key2, Value::Boolean(false));
        assert!(inserted);
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
            let obj_ref = gc.alloc(format!("key{}", i));
            let key = Value::Obj(obj_ref);
            let inserted = table.insert(key, Value::Number(f64::from(i)));
            assert!(inserted);
            let ret = table.get(key);
            assert_eq!(ret, Some(Value::Number(f64::from(i))));
        }
        assert_eq!(table.count, 2049);
        assert_eq!(table.capacity, 4096);
    }

    #[test]
    fn do_not_copy_tombstones_on_growth() {
        let mut gc = Gc::new();
        let mut table: HashTable = HashTable::new();
        for i in 0..5 {
            let obj_ref = gc.alloc(format!("key{}", i));
            let key = Value::Obj(obj_ref);
            table.insert(key, Value::Number(f64::from(i)));
            table.delete(key);
        }
        assert_eq!(table.count, 5);
        assert_eq!(table.capacity, 8);
        for i in 6..14 {
            let obj_ref = gc.alloc(format!("key{}", i));
            let key = Value::Obj(obj_ref);
            table.insert(key, Value::Number(f64::from(i)));
        }
        assert_eq!(table.count, 8);
        assert_eq!(table.capacity, 16);
    }

    #[test]
    fn handle_tombstones_correctly() {
        let mut gc = Gc::new();
        // all those keys have hash % 8 == 2
        let key1 = Value::Obj(gc.alloc("3".to_string()));
        let value1 = Value::Number(1.0);
        let key2 = Value::Obj(gc.alloc("12".to_string()));
        let value2 = Value::Number(2.0);
        let key3 = Value::Obj(gc.alloc("23".to_string()));
        let value3 = Value::Number(3.0);

        // has hash % 8 == 3
        let key4 = Value::Obj(gc.alloc("key5".to_string()));
        let value4 = Value::Number(4.0);
        let mut table: HashTable = HashTable::new();

        table.insert(key1, value1);
        table.insert(key2, value2);
        table.insert(key3, value3);
        assert_eq!(table.count, 3);

        table.delete(key2);
        assert!(table.get(key2).is_none());
        assert_eq!(table.get(key3), Some(value3));
        assert_eq!(table.count, 3);

        table.insert(key4, value4);
        assert!(table.get(key2).is_none());
        assert_eq!(table.get(key3), Some(value3));
        assert_eq!(table.get(key4), Some(value4));
        assert_eq!(table.count, 3);
    }

    #[test]
    fn find_str() {
        let mut gc = Gc::new();
        // all those keys have hash % 8 == 2
        let key1 = Value::Obj(gc.alloc("3".to_string()));
        let value1 = Value::Number(1.0);
        let key2_obj = gc.alloc("12".to_string());
        let key2 = Value::Obj(key2_obj);
        let value2 = Value::Number(2.0);
        let key3 = Value::Obj(gc.alloc("23".to_string()));
        let value3 = Value::Number(3.0);

        let mut table: HashTable = HashTable::new();

        table.insert(key1, value1);
        table.insert(key2, value2);
        table.insert(key3, value3);

        let res = table.find_string("12").unwrap();
        assert_eq!(res, key2_obj);
    }
    #[test]
    fn add_all() {
        let mut gc = Gc::new();
        let mut from: HashTable = HashTable::new();
        for i in 0..2049 {
            let obj_ref = gc.alloc(format!("key{}", i));
            let key = Value::Obj(obj_ref);
            from.insert(key, Value::Number(f64::from(i)));
        }

        let mut to = HashTable::new();
        to.add_all(&from);
        assert_eq!(from.count, to.count);
        assert_eq!(from.capacity, to.capacity);
    }

    #[test]
    fn delete_existing() {
        let mut table = HashTable::new();
        let key = Value::Boolean(true);
        table.insert(key, Value::Boolean(true));

        let deleted = table.delete(key);
        assert!(deleted);
        assert_eq!(table.count, 1) // tombstone
    }

    #[test]
    fn delete_on_empty() {
        let mut table = HashTable::new();
        let key = Value::Boolean(true);

        let deleted = table.delete(key);
        assert!(!deleted);
        assert_eq!(table.count, 0)
    }

    #[test]
    fn delete_not_existing() {
        let mut table = HashTable::new();
        let key = Value::Boolean(true);
        table.insert(Value::Boolean(false), Value::Boolean(false));

        let deleted = table.delete(key);
        assert!(!deleted);
        assert_eq!(table.count, 1)
    }
}
