mod probabilistic;

use crate::config_parser::ConfigurationValue;
use crate::meta_pattern::{MetaPattern, MetaPatternBuilderArgument};
// use crate::meta_pattern::many_to_one_pattern::probabilistic::{UniformDistanceMultiPattern, UniformMultiPattern};
// pub mod probabilistic;

pub trait ManyToOnePattern: MetaPattern<ManyToOneParam, usize>{}
impl <T> ManyToOnePattern for T where T: MetaPattern<ManyToOneParam, usize>{}

#[allow(dead_code)]
pub struct ManyToOneParam{
    pub(crate) origin: Option<usize>,
    pub(crate) destination: Option<usize>,
    pub(crate) vector: Vec<usize>,
}

pub fn new_many_to_one_pattern(arg: MetaPatternBuilderArgument) -> Box<dyn ManyToOnePattern>
{
    if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
    {
        match cv_name.as_str() {
            // "Uniform" => Box::new(UniformMultiPattern::new(arg)),
            // "UniformDistance" => Box::new(UniformDistanceMultiPattern::new(arg)),
            _ => panic!("Unknown many_to_one_pattern {}", cv_name),
        }
    } else {
        panic!("ManyToOne should be created from an Object");
    }
}