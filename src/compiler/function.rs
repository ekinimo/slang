use crate::value::Value;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct ErrTrace {
    message: String,
    child: Option<Box<ErrTrace>>,
}

impl ErrTrace {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            child: None,
        }
    }

    pub fn wrap(self, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            child: Some(Box::new(self)),
        }
    }
}

/// Type for functions that can be executed by the runtime
pub type FunctionType = Box<dyn for<'a> Fn(&'a mut Vec<Value>) -> Result<(), ErrTrace> + 'static>;

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
        F: for<'a> Fn(&'a mut Vec<Value>) -> Result<(), ErrTrace> + 'static,
    {
        CompiledFunction {
            inner: Rc::new(RefCell::new(Box::new(f))),
            param_count,
        }
    }

    /// Call the function with the provided memory context and parameter base
    pub fn call(&self, mem: &mut Vec<Value>) -> Result<(), ErrTrace> {
        let closure = self.inner.borrow();
        //println!("\t\tbefore call :");
        //println!("\t\t\t\tmem     : {mem:?}");
        let ret = closure(mem);
        //println!("\t\tafter  call :");
        //println!("\t\t\t\tmem     : {mem:?}");
        //println!("\t\t\t\tresult  : {ret:?}");

        ret
    }
}
