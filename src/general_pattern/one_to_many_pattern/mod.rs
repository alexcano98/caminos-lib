pub mod neighbours;

use crate::config_parser::ConfigurationValue;
use crate::general_pattern::{GeneralPattern, GeneralPatternBuilderArgument};

/// A 'OneToManyPattern' is a pattern that takes a single natural number (usize), and returns a list of natural numbers.
/// The source_size and destination_size of the fn initialize (...) method represent where those are natural number exist.
/// This is useful for patterns that return a list of neighbours, for example.
pub trait OneToManyPattern: GeneralPattern<usize, Vec<usize>>{}
impl <T> OneToManyPattern for T where T: GeneralPattern<usize, Vec<usize>>{}


pub(super) fn new_one_to_many_pattern(arg: GeneralPatternBuilderArgument) -> Box<dyn OneToManyPattern>
{
    if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
    {
        match cv_name.as_str() {
            "ManhattanNeighbours" => Box::new(neighbours::ManhattanNeighbours::new(arg)),
            "KingNeighbours" => Box::new(neighbours::KingNeighbours::new(arg)),
            "HypercubeNeighbours" => Box::new(neighbours::HypercubeNeighbours::new(arg)),
            "BinomialTreeNeighbours" => Box::new(neighbours::BinomialTreeNeighbours::new(arg)),
            "BinaryTreeNeighbours" => Box::new(neighbours::BinaryTreeNeighbours::new(arg)),
            "AllNeighbours" => Box::new(neighbours::AllNeighbours::new(arg)),
            "ImmediateNeighbours" => Box::new(neighbours::ImmediateNeighbours::new(arg)),
            _ => panic!("Unknown OneToManyPattern {}", cv_name),
        }
    } else {
        panic!("OneToManyOattern must be an Object");
    }
}