use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct NullMapper;

impl NullMapper {
    pub fn new(_ctx: &mut impl super::Context) -> Self {
        Self
    }
}

impl super::MapperTrait for NullMapper {}
