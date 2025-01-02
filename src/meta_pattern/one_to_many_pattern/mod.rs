pub mod neighbours;

use crate::config_parser::ConfigurationValue;
use crate::meta_pattern::{MetaPattern, MetaPatternBuilderArgument};


pub trait OneToManyPattern: MetaPattern<usize, Vec<usize>>{}
impl <T> OneToManyPattern for T where T: MetaPattern<usize, Vec<usize>>{}


pub(super) fn new_one_to_many_pattern(arg: MetaPatternBuilderArgument) -> Box<dyn OneToManyPattern>
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
            "InmediateNeighbours" => Box::new(neighbours::InmediateNeighbours::new(arg)),
            _ => panic!("Unknown OneToManyPattern {}", cv_name),
        }
    } else {
        panic!("OneToManyOattern must be an Object");
    }
}