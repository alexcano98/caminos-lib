// use quantifiable_derive::Quantifiable;
// use rand::prelude::StdRng;
// use rand::Rng;
// use crate::match_object_panic;
// use crate::meta_pattern::{MetaPattern, MetaPatternBuilderArgument};
// use crate::topology::Topology;
// use crate::ConfigurationValue;
// use crate::meta_pattern::many_to_one_pattern::ManyToOneParam;
//
// #[derive(Quantifiable, Debug)]
// pub struct UniformMultiPattern {
//     allow_source_destination: bool,
//     size: usize,
// }
//
// impl MetaPattern<ManyToOneParam, usize> for UniformMultiPattern {
//
//     fn initialize(&mut self, _source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
//         self.size = target_size;
//     }
//
//     fn get_destination(&self, param: ManyToOneParam, _topology: Option<&dyn Topology>, rng: &mut StdRng) -> usize {
//
//         match param {
//             ManyToOneParam::Vector { vector } => {
//                 //remove origin and destination from vector if they exist and are not allowed
//                 let r=rng.gen_range(0..vector.len());
//                 vector[r]
//             },
//             ManyToOneParam::Pair { origin, destination } => {
//                 let random_size = if !self.allow_source_destination { self.size-2 } else { self.size };
//                 let r=rng.gen_range(0..random_size);
//                 if self.allow_source_destination {
//                     r
//                 } else {
//                     let r = if r >= origin || (r >= destination && r+1 >= origin)
//                     {
//                         r+1
//                     }else {
//                         r
//                     };
//
//                     let r = if r >= destination
//                     {
//                         r+1
//                     }else {
//                         r
//                     };
//                     r
//                 }
//             },
//         }
//     }
// }
//
// impl UniformMultiPattern {
//     pub fn new(arg: MetaPatternBuilderArgument) -> UniformMultiPattern {
//         let mut allow_source_destination = false;
//         match_object_panic!(arg.cv,"Uniform",value,
// 			"allow_source_destination" => allow_source_destination=value.as_bool().expect("bad value for allow_self"),
//         );
//         UniformMultiPattern { allow_source_destination, size: 0 }
//     }
//
//     pub fn get_basic_uniform_multi_pattern()-> UniformMultiPattern
//     {
//         UniformMultiPattern { allow_source_destination: false, size: 0 }
//     }
// }
//
// #[derive(Quantifiable, Debug)]
// pub struct UniformDistanceMultiPattern {
//     size: usize,
//     distance: usize,
// }
//
// impl MetaPattern<ManyToOneParam, usize> for UniformDistanceMultiPattern {
//
//     fn initialize(&mut self, _source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
//         self.size = target_size;
//     }
//
//     fn get_destination(&self, param: ManyToOneParam, topology: Option<&dyn Topology>, rng: &mut StdRng) -> usize {
//         if let Some(topology) = topology {
//             if self.distance > topology.diameter() {
//                 panic!("The distance is greater than the diameter of the topology");
//             }
//
//             if let ManyToOneParam::Pair { origin, destination } = param {
//
//                 let mut r = rng.gen_range(0..self.size);
//                 while topology.distance(r, origin) > self.distance || topology.distance(r, destination) > self.distance {
//                     r = rng.gen_range(0..self.size);
//                 }
//                 r
//
//             } else {
//                 panic!("The param of UniformDistanceMultiPattern must be a pair");
//             }
//         } else {
//             panic!("A topology its needed for UniformDistanceMultiPattern")
//         }
//     }
// }
//
// impl UniformDistanceMultiPattern {
//     pub fn new(arg: MetaPatternBuilderArgument) -> UniformDistanceMultiPattern {
//         let mut distance = None;
//         match_object_panic!(arg.cv,"UniformDistance",value,
//             "distance" => distance= Some(value.as_usize().expect("bad value for distance")),
//         );
//         let distance = distance.expect("distance is required");
//         UniformDistanceMultiPattern { size: 0, distance }
//     }
// }
//
//
//
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use rand::SeedableRng;
//
//     #[test]
//     fn test_uniform() {
//         let mut rng = StdRng::seed_from_u64(0);
//         let mut uniform = UniformMultiPattern { allow_source_destination: false, size: 0 };
//         // uniform.initialize(5, 5, None, &mut rng);
//         // let dest1 = uniform.get_destination(0, 0, None, &mut rng);
//         // assert_ne!(dest1, 0);
//         // let dest2 = uniform.get_destination(0, 1, None, &mut rng);
//         // assert_ne!(dest2, 0);
//         // assert_ne!(dest2, 1);
//         // let dest3 = uniform.get_destination(0, 2, None, &mut rng);
//         // assert_ne!(dest3, 0);
//         // assert_ne!(dest3, 2);
//         // let dest4 = uniform.get_destination(0, 3, None, &mut rng);
//         // assert_ne!(dest4, 0);
//         // assert_ne!(dest4, 3);
//         // let dest5 = uniform.get_destination(0, 4, None, &mut rng);
//         // assert_ne!(dest5, 0);
//         // assert_ne!(dest5, 4);
//         //
//         // let dest1 = uniform.get_destination(2, 0, None, &mut rng);
//         // assert_ne!(dest3, 2);
//         // assert_ne!(dest1, 0);
//         // let dest2 = uniform.get_destination(2, 1, None, &mut rng);
//         // assert_ne!(dest2, 2);
//         // assert_ne!(dest2, 1);
//         // let dest3 = uniform.get_destination(2, 2, None, &mut rng);
//         // assert_ne!(dest3, 2);
//         // let dest4 = uniform.get_destination(2, 3, None, &mut rng);
//         // assert_ne!(dest4, 2);
//         // assert_ne!(dest4, 3);
//         // let dest5 = uniform.get_destination(2, 4, None, &mut rng);
//         // assert_ne!(dest5, 2);
//         // assert_ne!(dest5, 4);
//     }
// }
