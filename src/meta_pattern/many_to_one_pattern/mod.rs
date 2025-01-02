mod probabilistic;

use crate::config_parser::ConfigurationValue;
use crate::meta_pattern::{GeneralPattern, MetaPatternBuilderArgument};

/// A 'ManyToOnePattern' is a pattern that takes a single natural number (usize), and returns a list of natural numbers.
/// The source_size and destination_size of the fn initialize (...) method represent where those are natural number exist.
/// Maybe not useful because of the ManyToManyPattern.
pub trait ManyToOnePattern: GeneralPattern<ManyToOneParam, usize>{}
impl <T> ManyToOnePattern for T where T: GeneralPattern<ManyToOneParam, usize>{}

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