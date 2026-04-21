use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use crate::general_pattern::{GeneralPattern, GeneralPatternBuilderArgument};
use crate::general_pattern::many_to_many_pattern::{new_many_to_many_pattern, ManyToManyParam, ManyToManyPattern};
use crate::general_pattern::one_to_many_pattern::{new_one_to_many_pattern, OneToManyPattern};
use crate::match_object_panic;
use crate::topology::Topology;
use crate::ConfigurationValue;



#[derive(Quantifiable, Debug)]
pub struct Composition{
    first: Box<dyn OneToManyPattern>,
    patterns: Vec< Box<dyn ManyToManyPattern>>,
}

impl GeneralPattern<usize, Vec<usize>>  for Composition {

    fn initialize(&mut self, source_size: usize, target_size: usize, topology: Option<&dyn Topology>, rng: &mut StdRng) {
        self.first.initialize(source_size, target_size, topology, rng);
        for pattern in self.patterns.iter_mut() {
            pattern.initialize(source_size, target_size, topology, rng);
        }
    }

    fn get_destination(&self, param: usize, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Vec<usize> {
        let mut param = ManyToManyParam{ list: self.first.get_destination(param, topology, rng), ..Default::default()};
        let mut list = vec![];
        for pattern in self.patterns.iter() {
            list = pattern.get_destination(param.clone(), topology, rng);
            param.list = list.clone();
        }
       list
    }
}

impl Composition{
    pub fn new(arg: GeneralPatternBuilderArgument) -> Composition {
        let mut first = None;
        let mut patterns = None;

        match_object_panic!(arg.cv,"Composition",value,
            "first" => first = Some( new_one_to_many_pattern(GeneralPatternBuilderArgument{cv:value,..arg}) ),
            "many_to_many_patterns" => patterns = Some(value.as_array().expect("bad value for patterns").into_iter().map(|x| new_many_to_many_pattern(GeneralPatternBuilderArgument{cv:x,..arg})).collect()),
        );

        let first = first.expect("first not found");
        let patterns = patterns.expect("patterns not found");
        Composition { first, patterns }
    }
}