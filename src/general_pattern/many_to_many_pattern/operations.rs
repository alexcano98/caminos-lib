use std::collections::HashSet;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use crate::match_object_panic;
use crate::ConfigurationValue;
use crate::general_pattern::many_to_many_pattern::{new_many_to_many_pattern, ManyToManyParam, ManyToManyPattern};
use crate::general_pattern::{GeneralPattern, GeneralPatternBuilderArgument};
use crate::topology::Topology;

/**
Pattern that iters through patterns, and the output vector of a pattern is the input vector of the next pattern.
The other params (source and destination fields) are kept the same. Only affects the list field.
```ignore
    Composition {
        many_to_many_patterns:[ RandomFilter{elements_to_return: 1,}, DistanceFilter{ distance: 1,}]
    }
```
**/

#[derive(Quantifiable, Debug)]
pub struct Composition {
    patterns: Vec< Box<dyn ManyToManyPattern>>
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for Composition {

    fn initialize(&mut self, source_size: usize, target_size: usize, topology: Option<&dyn Topology>, rng: &mut StdRng) {
        for pattern in self.patterns.iter_mut() {
            pattern.initialize(source_size, target_size, topology, rng);
        }
    }

    fn get_destination(&self, param: ManyToManyParam, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Vec<usize> {
        let mut list = param.list;
        for pattern in self.patterns.iter() {
            list = pattern.get_destination(ManyToManyParam { list: list, ..param }, topology, rng);
        }
        list
    }
}

impl Composition {
    pub fn new(arg: GeneralPatternBuilderArgument) -> Composition {
        let mut patterns = None;
        match_object_panic!(arg.cv,"Composition",value,
            "many_to_many_patterns" => {
                patterns = Some(value.as_array().expect("bad value for patterns").into_iter().map(|x| new_many_to_many_pattern(GeneralPatternBuilderArgument{cv:x,..arg})).collect());
            }
        );
        let patterns = patterns.expect("patterns not found");
        Composition { patterns }
    }
}

/**
Pattern that performs the union operation between the output vectors of the patterns for each source element.
```ignore
    Sum{
        many_to_many_patterns:[],
    },
```
**/
#[derive(Quantifiable, Debug)]
pub struct Sum{
    patterns: Vec< Box<dyn ManyToManyPattern>>,
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for Sum {

    fn initialize(&mut self, source_size: usize, target_size: usize, topology: Option<&dyn Topology>, rng: &mut StdRng) {
        for pattern in self.patterns.iter_mut() {
            pattern.initialize(source_size, target_size, topology, rng);
        }
    }

    fn get_destination(&self, param: ManyToManyParam, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Vec<usize> {
        let mut list = HashSet::new();
        for pattern in self.patterns.iter() {
            list.extend(pattern.get_destination(param.clone(), topology, rng));
        }
        list.into_iter().collect()
    }
}

impl Sum{
    pub fn new(arg: GeneralPatternBuilderArgument) -> Sum {
        let mut patterns = None;
        match_object_panic!(arg.cv,"Sum",value,
            "many_to_many_patterns" => {
                patterns = Some(value.as_array().expect("bad value for patterns").into_iter().map(|x| new_many_to_many_pattern(GeneralPatternBuilderArgument{cv:x,..arg})).collect());
            }
        );
        let patterns = patterns.expect("patterns not found");
        Sum { patterns }
    }
}


#[cfg(test)]
mod tests{
    use rand::prelude::StdRng;
    use rand::SeedableRng;
    use crate::config_parser::ConfigurationValue;
    use crate::general_pattern::many_to_many_pattern::filters::{DistanceFilter, RandomFilter};
    use crate::general_pattern::many_to_many_pattern::ManyToManyParam;
    use crate::general_pattern::many_to_many_pattern::operations::Composition;
    use crate::general_pattern::GeneralPattern;
    use crate::Plugs;
    use crate::topology::{new_topology, Topology, TopologyBuilderArgument};

    #[allow(dead_code)]
    fn get_hamming_topology(switches: f64) -> Box<dyn Topology>{
        let hamming_cv = ConfigurationValue::Object("Hamming".to_string(), vec![
            ("servers_per_router".to_string(), ConfigurationValue::Number(1.0)),
            ("sides".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(switches)]),),
        ]); //hamming CV
        let plugs = Plugs::default();
        let params = TopologyBuilderArgument {
            cv: &hamming_cv,
            rng: &mut StdRng::seed_from_u64(0),
            plugs: &plugs,
        };
        new_topology(params)
    }

    #[test]
    fn test_composition() {
        let mut rng = StdRng::seed_from_u64(0);
        let binding = get_hamming_topology(4.0);
        let topo= Some(binding.as_ref());
        let param = ManyToManyParam { origin: Some(0), destination: Some(1), list: (0..4).collect() };

        let mut composition_0 = Composition {
            patterns: vec![
                Box::new(DistanceFilter::get_basic_distance_filter(0)),
                Box::new(RandomFilter::get_basic_random_filter()),
            ]
        };

        composition_0.initialize(4, 4, topo, &mut rng);
        assert_eq!(composition_0.get_destination(param.clone(), topo, &mut rng), vec![]);

        let mut composition_1 = Composition {
            patterns: vec![
                Box::new(DistanceFilter::get_basic_distance_filter(1)),
                Box::new(RandomFilter::get_basic_random_filter()),
            ]
        };
        composition_1.initialize(4, 4, topo.clone(), &mut rng);
        assert_eq!(composition_1.get_destination(param.clone(), topo.clone(), &mut rng).len(), 1);

    }
}