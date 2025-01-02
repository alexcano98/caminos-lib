pub mod probabilistic;

use crate::config_parser::ConfigurationValue;
use crate::meta_pattern::{MetaPattern, MetaPatternBuilderArgument};

pub struct ManyToManyParam{
    pub(crate) origin: Option<usize>,
    pub(crate) destination: Option<usize>,
    pub(crate) vector: Vec<usize>,
}

pub trait ManyToManyPattern: MetaPattern<ManyToManyParam, Vec<usize>>{}
impl <T> ManyToManyPattern for T where T: MetaPattern<ManyToManyParam, Vec<usize>>{}


pub fn new_many_to_many_pattern(arg: MetaPatternBuilderArgument) -> Box<dyn ManyToManyPattern>
{
    if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
    {
        match cv_name.as_str() {
            "Uniform" => Box::new(probabilistic::Uniform::new(arg)),
            "UniformDistance" => Box::new(probabilistic::UniformDistance::new(arg)),
            _ => panic!("Unknown many_to_many_pattern {}", cv_name),
        }
    } else {
        panic!("ManyToManyPattern should be created from an Object");
    }
}