use crate::types::{obj::Obj, value::Value};

use super::VM;

impl VM {
    pub(super) fn define_native_functions(&mut self) {
        self.define_native("clock", |_, _, _| {
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
            Value::Number(millis)
        });
        self.define_native("heapdump", |_, _, vm| {
            vm.heapdump();
            Value::Nil
        });
        self.define_native("gc", |_, _, vm| {
            vm.collect_garbage();
            Value::Nil
        });
    }

    fn define_native(&mut self, name: &str, function: fn(u8, *mut Value, &mut VM) -> Value) {
        let name = self.alloc(name);
        self.push(Value::Obj(name));
        let function = self.alloc(Obj::Native(function));
        self.push(Value::Obj(function));
        self.globals.insert(self.peek(1), self.peek(0));
        self.pop();
        self.pop();
    }
}
