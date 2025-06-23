pub mod filters;
mod operations;
mod resource_selection;

use crate::config_parser::ConfigurationValue;
use crate::general_pattern::{GeneralPattern, GeneralPatternBuilderArgument};

#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct ManyToManyParam{
    pub(crate) origin: Option<usize>,
    pub(crate) current: Option<usize>, //is it important?
    pub(crate) destination: Option<usize>,
    pub(crate) list: Vec<usize>,
    pub(crate) extra: Option<usize>,
}

/// A 'ManyToManyPattern' is a pattern that takes a ManyToManyParam, and returns a list of natural numbers.
/// A ManyToManyParam is a struct that can be interpreted as a list of natural numbers.
/// It contains two Option<usize> fields, origin and destination, and a Vec<usize> field, vector.
/// The source_size and destination_size of the fn initialize (...) method represent where those are natural number exist.
pub trait ManyToManyPattern: GeneralPattern<ManyToManyParam, Vec<usize>>{}
impl <T> ManyToManyPattern for T where T: GeneralPattern<ManyToManyParam, Vec<usize>>{}


pub fn new_many_to_many_pattern(arg: GeneralPatternBuilderArgument) -> Box<dyn ManyToManyPattern>
{
    if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
    {
        match cv_name.as_str() {
            "IdentityFilter" => Box::new(filters::IdentityFilter::new(arg)),
            "RandomFilter" => Box::new(filters::RandomFilter::new(arg)),
            "DistanceFilter" => Box::new(filters::DistanceFilter::new(arg)),
            "SubplaneFilter" => Box::new(filters::SubplaneFilter::new(arg)),
            "MinFilter" => Box::new(filters::MinFilter::new(arg)),
            "Composition" => Box::new(operations::Composition::new(arg)),
            "Sum" => Box::new(operations::Sum::new(arg)),
            "ConsecutiveSelection" => Box::new(resource_selection::ConsecutiveSelection::new(arg)),
            "RandomSelection" => Box::new(resource_selection::RandomSelection::new(arg)),
            "BlockSelection" => Box::new(resource_selection::BlockSelection::new(arg)),
            "LTileSelection" => Box::new(resource_selection::LTileSelection::new(arg)),
            "DiagonalSelection" => Box::new(resource_selection::DiagonalSelection::new(arg)),
            "IterBlockSelection" => Box::new(resource_selection::IterBlockSelection::new(arg)),
            _ => panic!("Unknown many_to_many_pattern {}", cv_name),
        }
    } else {
        panic!("ManyToManyPattern should be created from an Object");
    }
}