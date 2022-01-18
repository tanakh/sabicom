use std::{cell::RefCell, rc::Rc};

pub type Ref<T> = Rc<RefCell<T>>;

pub fn wrap_ref<T>(v: T) -> Ref<T> {
    Rc::new(RefCell::new(v))
}

pub fn clone_ref<T>(v: &Ref<T>) -> Ref<T> {
    Rc::clone(v)
}
