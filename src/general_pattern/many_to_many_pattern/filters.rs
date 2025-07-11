use quantifiable_derive::Quantifiable;
use rand::prelude::*;
use crate::match_object_panic;
use crate::general_pattern::{GeneralPattern, GeneralPatternBuilderArgument};
use crate::topology::Topology;
use crate::ConfigurationValue;
use crate::general_pattern::many_to_many_pattern::ManyToManyParam;
use crate::topology::cartesian::CartesianData;

/**
Pattern that returns exactly the same elements that are passed as argument.
```ignore
    IdentityFilter {}
```
**/
#[derive(Quantifiable, Debug)]
pub struct IdentityFilter {}
impl GeneralPattern<ManyToManyParam, Vec<usize>> for IdentityFilter {
    fn initialize(&mut self, _source_size: usize, _target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {}

    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        param.list.clone()
    }
}

impl IdentityFilter {
    pub fn new(_arg: GeneralPatternBuilderArgument) -> IdentityFilter {
        IdentityFilter {}
    }

    pub fn get_basic_identity_filter() -> IdentityFilter {
        IdentityFilter {}
    }
}


/**
Pattern that returns random elements from a vector passed as argument.
```ignore
    RandomFilter {
        elements_to_return: 1, //number of random elements selected
        source: true, // (Optional) if true, source is not considered.
        destination: false, // (Optional) if true, destination is considered. Else it is considered.
    }
```
**/

#[derive(Quantifiable, Debug)]
pub struct RandomFilter {
    elements_to_return: usize,
    source: bool,
    destination: bool,
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for RandomFilter {

    fn initialize(&mut self, _source_size: usize, _target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {}
    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, rng: &mut StdRng) -> Vec<usize> {

        let list = if self.source {
            param.list.clone().into_iter().filter(|x| x != param.destination.as_ref().unwrap()).collect::<Vec<usize>>()
        } else {
            param.list.clone()
        };

        let mut list = if self.destination {
            list.into_iter().filter(|x| x != param.origin.as_ref().unwrap()).collect::<Vec<usize>>()
        } else {
            list
        };

        list.shuffle(rng);
        list.into_iter().take(self.elements_to_return).collect()
    }
}

impl RandomFilter {
    pub fn new(arg: GeneralPatternBuilderArgument) -> RandomFilter {
        let mut elements_to_return = 1;
        let source= true;
        let destination= true;
        match_object_panic!(arg.cv,"RandomFilter",value,
            "elements_to_return" => elements_to_return=value.as_usize().expect("bad value for elements_to_return"),
        );
        RandomFilter {elements_to_return, source, destination}
    }

    pub fn get_basic_random_filter() -> RandomFilter
    {
        RandomFilter { elements_to_return: 1, source: true, destination: true }
    }
}


/**
Pattern that returns elements that are at a certain distance from the origin and destination.
```ignore
    DistanceFilter {
        distance: 1, //distance
        source: true, // (Optional) if true, the intermediates should be at distance from the origin.
        destination: false, // (Optional) if true, the intermediates should be at distance from the destination. Else is not considered.
    }
```
**/
#[derive(Quantifiable, Debug)]
pub struct DistanceFilter {
    distance: usize,
    source: bool,
    destination: bool,
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for DistanceFilter {

    fn initialize(&mut self, _source_size: usize, _target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {}

    fn get_destination(&self, param: ManyToManyParam, topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        if let Some(topology) = topology {
            if self.distance > topology.diameter() {
                panic!("The distance is greater than the diameter of the topology");
            }
            let mut list = Vec::new();
            let origin = param.origin.unwrap();
            let destination = param.destination.unwrap();
            for z in param.list.iter() {
                if (!self.source || topology.distance(*z, origin) == self.distance) && (!self.destination || topology.distance(*z, destination) == self.distance) {
                    list.push(*z);
                }
            }
            list
        } else {
            panic!("A topology its needed for UniformDistance")
        }
    }
}

impl DistanceFilter {
    pub fn new(arg: GeneralPatternBuilderArgument) -> DistanceFilter {
        let mut distance = None;
        let mut source= true;
        let mut destination= true;
        match_object_panic!(arg.cv,"UniformDistance",value,
            "distance" => distance= Some(value.as_usize().expect("bad value for distance")),
            "source" => source= value.as_bool().expect("bad value for source"),
            "destination" => destination= value.as_bool().expect("bad value for destination"),
        );
        let distance = distance.expect("distance is required");
        DistanceFilter { distance, source, destination }
    }

    pub fn get_basic_distance_filter(distance:usize) -> DistanceFilter{
        DistanceFilter{distance, source: true, destination: true}
    }
}

/**
Pattern which discard elements which are in the same sub plane than the origin or destination.
```ignore
    SubPlaneFilter {
        sides: [10, 10, 10],
        subplanes: [[0, 1, 1], [1, 0, 1], [1, 1, 0]], //subplanes yz, xz, xy
    }
```
**/

#[derive(Quantifiable, Debug)]
pub struct SubplaneFilter {
    sides: Vec<usize>,
    subplanes: Vec<Vec<usize>>,
    cartesian_data: CartesianData,
    source: bool,
    destination: bool,
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for SubplaneFilter {

    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        if self.sides.iter().product::<usize>() != target_size{
            panic!("The target size is not the product of the sides");
        }
        if self.sides.iter().product::<usize>() != source_size{
            panic!("The source size is not the product of the sides");
        }
    }

    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        let source_coord = self.cartesian_data.unpack(param.origin.unwrap());
        let destination_coord = self.cartesian_data.unpack(param.destination.unwrap());
        let mut list = param.list.clone();

        for vector in &self.subplanes{
            let mul_origin = source_coord.iter().zip(vector.iter()).map(|(x,y)| x*y).collect::<Vec<usize>>();
            let mul_destination = destination_coord.iter().zip(vector.iter()).map(|(x,y)| x*y).collect::<Vec<usize>>();

            for z in list.clone().into_iter(){
                let coord = self.cartesian_data.unpack(z);
                let mul = coord.iter().zip(vector.iter()).map(|(x,y)| x*y).collect::<Vec<usize>>();
                // println!("{:?} {:?} {:?}", mul_origin, mul_destination, mul);
                if list.contains(&z) && ((self.source && mul == mul_origin) || (self.destination && mul == mul_destination)){
                    list.retain(|&x| x != z);
                }
            }
        }
        list
    }
}

impl SubplaneFilter {
    pub fn new(arg: GeneralPatternBuilderArgument) -> SubplaneFilter {
        let mut sides = None;
        let mut subplanes = None;
        let mut source= true;
        let mut destination= true;
        match_object_panic!(arg.cv,"SubplaneFilter",value,
            "sides" => sides= Some(value.as_array().expect("bad value for sides").into_iter().map(|x| x.as_usize().expect("bad value for side")).collect()),
            "subplanes" => subplanes= Some(value.as_array().expect("bad value for subplanes").into_iter().map(|x| x.as_array().expect("bad value for subplane").into_iter().map(|y| y.as_usize().expect("bad value for subplane")).collect()).collect()),
            "source" => source= value.as_bool().expect("bad value for source"),
            "destination" => destination= value.as_bool().expect("bad value for destination"),
        );
        let sides: Vec<usize> = sides.expect("sides is required");
        let subplanes = subplanes.expect("subplanes is required");
        let cartesian_data = CartesianData::new(&sides);

        SubplaneFilter { sides, subplanes, cartesian_data, source, destination }
    }

    pub fn get_basic_sub_plane_filter(sides: Vec<usize>, subplanes: Vec<Vec<usize>>) -> SubplaneFilter {
        let cartesian_data = CartesianData::new(&sides);
        SubplaneFilter { sides, cartesian_data, subplanes, source: true, destination: true }
    }
}


/**
Pattern that returns the lowest element
```ignore
    Min {}
```
**/
#[derive(Quantifiable, Debug)]
pub struct MinFilter {}
impl GeneralPattern<ManyToManyParam, Vec<usize>> for MinFilter {
    fn initialize(&mut self, _source_size: usize, _target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {}

    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        if param.list.len() == 0{
            vec![]
        }else {
            vec![param.list.iter().min().unwrap().clone()]
        }
    }
}

impl MinFilter {
    pub fn new(_arg: GeneralPatternBuilderArgument) -> MinFilter {
        MinFilter {}
    }

    pub fn get_basic_identity_filter() -> MinFilter {
        MinFilter {}
    }
}


#[cfg(test)]
mod tests {
    use std::default::Default;
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_uniform() {
        let mut rng = StdRng::seed_from_u64(0);
        let mut uniform = RandomFilter {elements_to_return: 1, source: true, destination: true };
        uniform.initialize(0, 10, None, &mut rng);
        let param = ManyToManyParam { origin: Some(0), destination: Some(1), list: (0..10).collect(), ..Default::default() };
        let destination = uniform.get_destination(param, None, &mut rng);
        assert_eq!(destination.len(), 1);

        let mut uniform = RandomFilter { elements_to_return: 3, source: true, destination: true };
        uniform.initialize(0, 10, None, &mut rng);
        let param = ManyToManyParam { origin: Some(0), destination: Some(3), list: (0..10).collect(), ..Default::default() };
        let destination = uniform.get_destination(param, None, &mut rng);
        assert_eq!(destination.len(), 3);
    }

    #[test]
    fn test_subplane_filter(){
        let mut rng = StdRng::seed_from_u64(0);
        let mut subplane = SubplaneFilter::get_basic_sub_plane_filter(vec![10,10,10], vec![vec![0,1,1], vec![1,0,1], vec![1,1,0]]);
        subplane.initialize(1000, 1000, None, &mut rng);
        let param = ManyToManyParam { origin: Some(0), destination: Some(1), list: (0..1000).collect(), ..Default::default() };
        let destination = subplane.get_destination(param, None, &mut rng);
        assert_eq!(destination.len(), 954);
    }
    #[test]
    fn test_min_filter(){
        let mut rng = StdRng::seed_from_u64(0);
        let mut min = MinFilter{};
        min.initialize(100, 100, None, &mut rng);
        let param = ManyToManyParam { list: (0..10).collect(), ..Default::default() };
        assert_eq!(vec![0], min.get_destination(param, None, &mut rng) );

    }

}
