pub mod filters;
mod operations;

use crate::config_parser::ConfigurationValue;
use crate::meta_pattern::{GeneralPattern, MetaPatternBuilderArgument};

#[derive(Clone)]
pub struct ManyToManyParam{
    pub(crate) origin: Option<usize>,
    // pub(crate) current: Option<usize>, //is it important?
    pub(crate) destination: Option<usize>,
    pub(crate) list: Vec<usize>,
}

/// A 'ManyToManyPattern' is a pattern that takes a ManyToManyParam, and returns a list of natural numbers.
/// A ManyToManyParam is a struct that can be interpreted as a list of natural numbers.
/// It contains two Option<usize> fields, origin and destination, and a Vec<usize> field, vector.
/// The source_size and destination_size of the fn initialize (...) method represent where those are natural number exist.
pub trait ManyToManyPattern: GeneralPattern<ManyToManyParam, Vec<usize>>{}
impl <T> ManyToManyPattern for T where T: GeneralPattern<ManyToManyParam, Vec<usize>>{}


pub fn new_many_to_many_pattern(arg: MetaPatternBuilderArgument) -> Box<dyn ManyToManyPattern>
{
    if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
    {
        match cv_name.as_str() {
            "IdentityFilter" => Box::new(filters::IdentityFilter::new(arg)),
            "RandomFilter" => Box::new(filters::RandomFilter::new(arg)),
            "DistanceFilter" => Box::new(filters::DistanceFilter::new(arg)),
            "SubplaneFilter" => Box::new(filters::SubplaneFilter::new(arg)),
            "Composition" => Box::new(operations::Composition::new(arg)),
            "Sum" => Box::new(operations::Sum::new(arg)),
            _ => panic!("Unknown many_to_many_pattern {}", cv_name),
        }
    } else {
        panic!("ManyToManyPattern should be created from an Object");
    }
}