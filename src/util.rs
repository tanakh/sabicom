use serde::{Deserialize, Serialize};

macro_rules! trait_alias {
    (pub trait $name:ident = $($traits:tt)+) => {
        pub trait $name: $($traits)* {}
        impl<T: $($traits)*> $name for T {}
    };
}
pub(crate) use trait_alias;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub pad: [Pad; 2],
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Pad {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    pub start: bool,
    pub select: bool,
}
