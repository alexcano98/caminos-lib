use quantifiable_derive::Quantifiable;
use rand::prelude::*;
use rand::Rng;
use crate::match_object_panic;
use crate::meta_pattern::{MetaPattern, MetaPatternBuilderArgument};
use crate::topology::Topology;
use crate::ConfigurationValue;
use crate::meta_pattern::many_to_many_pattern::ManyToManyParam;

#[derive(Quantifiable, Debug)]
pub struct Uniform {
    allow_source_destination: bool,
    size: usize,
    elements_to_return: usize,
}

impl MetaPattern<ManyToManyParam, Vec<usize>> for Uniform {

    fn initialize(&mut self, _source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        self.size = target_size;
    }

    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, rng: &mut StdRng) -> Vec<usize> {
        //get a random element from the vector and check if it is the origin or destination
        let list = if !self.allow_source_destination{
          param.vector.clone().into_iter().filter(|x| x != param.origin.as_ref().unwrap() && x != param.destination.as_ref().unwrap()).collect::<Vec<_>>()
        } else {
            param.vector
        };
        let mut list = list;
        list.shuffle(rng);
        list.into_iter().take(self.elements_to_return).collect()
    }
}

impl Uniform {
    pub fn new(arg: MetaPatternBuilderArgument) -> Uniform {
        let mut allow_source_destination = true;
        let mut elements_to_return = 1;
        match_object_panic!(arg.cv,"Uniform",value,
			"allow_source_destination" => allow_source_destination=value.as_bool().expect("bad value for allow_self"),
            "elements_to_return" => elements_to_return=value.as_usize().expect("bad value for elements_to_return"),
        );
        Uniform { allow_source_destination, size: 0, elements_to_return }
    }

    pub fn get_basic_uniform_meta_pattern() -> Uniform
    {
        Uniform { allow_source_destination: false, size: 0, elements_to_return: 1 }
    }
}

#[derive(Quantifiable, Debug)]
pub struct UniformDistance {
    size: usize,
    distance: usize,
    elements_to_return: usize,
}

impl MetaPattern<ManyToManyParam, Vec<usize>> for UniformDistance {

    fn initialize(&mut self, _source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        self.size = target_size;
    }

    fn get_destination(&self, param: ManyToManyParam, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Vec<usize> {
        if let Some(topology) = topology {
            if self.distance > topology.diameter() {
                panic!("The distance is greater than the diameter of the topology");
            }

            let mut list = Vec::new();
            for _ in 0..self.elements_to_return {
                let mut r = rng.gen_range(0..self.size);
                while topology.distance(r, param.origin.unwrap()) > self.distance || topology.distance(r, param.destination.unwrap()) > self.distance {
                    r = rng.gen_range(0..self.size);
                }
                list.push(r);
            }
            list
        } else {
            panic!("A topology its needed for UniformDistance")
        }
    }
}

impl UniformDistance {
    pub fn new(arg: MetaPatternBuilderArgument) -> UniformDistance {
        let mut distance = None;
        let mut elements_to_return = 1;
        match_object_panic!(arg.cv,"UniformDistance",value,
            "distance" => distance= Some(value.as_usize().expect("bad value for distance")),
            "elements_to_return" => elements_to_return=value.as_usize().expect("bad value for elements_to_return"),
        );
        let distance = distance.expect("distance is required");
        UniformDistance { size: 0, distance, elements_to_return }
    }
}




#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_uniform() {
        let mut rng = StdRng::seed_from_u64(0);
        let mut uniform = Uniform { allow_source_destination: false, size: 0, elements_to_return: 1 };
        uniform.initialize(0, 10, None, &mut rng);
        let param = ManyToManyParam { origin: Some(0), destination: Some(1), vector: (0..10).collect() };
        let destination = uniform.get_destination(param, None, &mut rng);
        assert_eq!(destination.len(), 1);

        let mut uniform = Uniform { allow_source_destination: false, size: 0, elements_to_return: 3 };
        uniform.initialize(0, 10, None, &mut rng);
        let param = ManyToManyParam { origin: Some(0), destination: Some(3), vector: (0..10).collect() };
        let destination = uniform.get_destination(param, None, &mut rng);
        assert_eq!(destination.len(), 3);
    }
}
