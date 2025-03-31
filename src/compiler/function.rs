use crate::value::Value;
use std::cell::RefCell;
use std::rc::Rc;

/// Type for functions that can be executed by the runtime
pub type FunctionType = Box<dyn for<'a> Fn(&'a mut Vec<Value>, usize) + 'static>;

#[derive(Clone)]
pub struct CompiledFunction {
    /// The actual function implementation
    pub inner: Rc<RefCell<FunctionType>>,
    
    /// Number of parameters this function expects
    pub param_count: usize,
}

impl CompiledFunction {
    /// Create a new compiled function
    pub fn new<F>(f: F, param_count: usize) -> Self
    where
        F: for<'a> Fn(&'a mut Vec<Value>, usize) + 'static,
    {
        CompiledFunction {
            inner: Rc::new(RefCell::new(Box::new(f))),
            param_count,
        }
    }

    /// Call the function with the provided memory context and parameter base
    pub fn call(&self, mem: &mut Vec<Value>, param_base: usize) {
        let closure = self.inner.borrow();
        closure(mem, param_base);
    }
}
