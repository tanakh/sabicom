use std::{cell::RefCell, rc::Rc};

pub type Ref<T> = Rc<RefCell<T>>;

pub fn wrap_ref<T>(v: T) -> Ref<T> {
    Rc::new(RefCell::new(v))
}

pub fn clone_ref<T: ?Sized>(v: &Ref<T>) -> Ref<T> {
    Rc::clone(v)
}

#[derive(Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

pub struct FrameBuffer {
    pub width: usize,
    pub height: usize,
    pub buf: Vec<Color>,
}

impl FrameBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buf: vec![Color::new(0, 0, 0); width * height],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Color {
        assert!(x < self.width);
        assert!(y < self.height);
        self.buf[y * self.width + x].clone()
    }

    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        assert!(x < self.width);
        assert!(y < self.height);
        self.buf[y * self.width + x] = color;
    }
}

pub struct Wire<T>(Ref<T>);

impl<T: Clone> Wire<T> {
    pub fn new(v: T) -> Self {
        Self(wrap_ref(v))
    }

    pub fn get(&self) -> T {
        self.0.borrow().clone()
    }

    pub fn set(&self, v: T) {
        *self.0.borrow_mut() = v;
    }
}

impl<T> Clone for Wire<T> {
    fn clone(&self) -> Self {
        Self(clone_ref(&self.0))
    }
}
