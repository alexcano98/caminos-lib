use crate::meta_pattern::simple_pattern::SimplePattern;
use crate::meta_pattern::simple_pattern::new_pattern;
use std::cell::{RefCell};
use std::collections::VecDeque;
use std::convert::TryInto;
use ::rand::{Rng,rngs::StdRng};
use std::fs::File;
use std::io::{BufRead,BufReader};
use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::topology::cartesian::CartesianData;//for CartesianTransform
use crate::topology::{Topology, Location};
use crate::{match_object_panic};
use crate::meta_pattern::MetaPattern;
use crate::meta_pattern::{MetaPatternBuilderArgument};


/**
A map read from file. Each node has a unique destination. See [RandomPermutation] for related matters.
The file is read at creation and should contain only lines with pairs `source destination`.

Example configuration:
```ignore
FileMap{
	/// Note this is a string literal.
	filename: "/path/to/meta_pattern",
	legend_name: "A meta_pattern in my device",
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct FileMap
{
    permutation: Vec<usize>,
}

impl MetaPattern<usize, usize> for FileMap
{
    fn initialize(&mut self, _source_size:usize, _target_size:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)
    {
        //self.permutation=(0..size).collect();
        //rng.shuffle(&mut self.permutation);
    }
    fn get_destination(&self, origin:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)->usize
    {
        self.permutation[origin]
    }
}

impl FileMap
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> FileMap
    {
        let mut filename=None;
        match_object_panic!(arg.cv,"FileMap",value,
			"filename" => filename = Some(value.as_str().expect("bad value for filename").to_string()),
		);
        let filename=filename.expect("There were no filename");
        let file=File::open(&filename).expect("could not open meta_pattern file.");
        let reader = BufReader::new(&file);
        let mut permutation=Vec::new();
        for rline in reader.lines()
        {
            let line=rline.expect("Some problem when reading the traffic meta_pattern.");
            let mut words=line.split_whitespace();
            let origin=words.next().unwrap().parse::<usize>().unwrap();
            let destination=words.next().unwrap().parse::<usize>().unwrap();
            while permutation.len()<=origin || permutation.len()<=destination
            {
                permutation.push((-1isize) as usize);//which value use as filler?
            }
            permutation[origin]=destination;
        }
        FileMap{
            permutation,
        }
    }
    pub(crate) fn embedded(arg:MetaPatternBuilderArgument) -> FileMap
    {
        let mut map = None;
        match_object_panic!(arg.cv,"EmbeddedMap",value,
			"map" => map = Some(value.as_array()
				.expect("bad value for map").iter()
				.map(|v|v.as_f64().expect("bad value for map") as usize).collect()),
		);
        let permutation = map.expect("There were no map");
        FileMap{
            permutation
        }
    }
}


///Divide the topology according to some given link classes, considering the graph components if the other links were removed.
///Then apply the `global_pattern` among the components and select randomly inside the destination component.
///Note that this uses the topology and will cause problems if used as a sub-meta_pattern.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ComponentsPattern
{
    component_classes: Vec<usize>,
    //block_pattern: Box<dyn Pattern>,//we would need patterns between places of different extent.
    global_pattern: Box<dyn SimplePattern>,
    components: Vec<Vec<usize>>,
}

impl MetaPattern<usize, usize> for ComponentsPattern
{
    fn initialize(&mut self, _source_size:usize, _target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        let topology=topology.expect("ComponentsPattern needs a topology");
        let mut allowed_components=vec![];
        for link_class in self.component_classes.iter()
        {
            if *link_class>=allowed_components.len()
            {
                allowed_components.resize(*link_class+1,false);
            }
            allowed_components[*link_class]=true;
        }
        self.components=topology.components(&allowed_components);
        //for (i,component) in self.components.iter().enumerate()
        //{
        //	println!("component {}: {:?}",i,component);
        //}
        self.global_pattern.initialize(self.components.len(),self.components.len(),Some(topology),rng);
    }
    fn get_destination(&self, origin:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        //let local=origin % self.block_size;
        //let global=origin / self.block_size;
        //let n=topology.num_routers();
        let topology=topology.expect("ComponentsPattern needs a topology");
        let router_origin=match topology.server_neighbour(origin).0
        {
            Location::RouterPort{
                router_index,
                router_port: _,
            } => router_index,
            _ => panic!("what origin?"),
        };
        let mut global=self.components.len();
        for (g,component) in self.components.iter().enumerate()
        {
            if component.contains(&router_origin)
            {
                global=g;
                break;
            }
        }
        if global==self.components.len()
        {
            panic!("Could not found component of {}",router_origin);
        }
        let global_dest=self.global_pattern.get_destination(global,Some(topology),rng);
        //let local_dest=self.block_pattern.get_destination(local,topology,rng);
        let r_local=rng.gen_range(0..self.components[global_dest].len());
        let dest=self.components[global_dest][r_local];
        let radix=topology.ports(dest);
        let mut candidate_stack=Vec::with_capacity(radix);
        for port in 0..radix
        {
            match topology.neighbour(dest,port).0
            {
                Location::ServerPort(destination) => candidate_stack.push(destination),
                _ => (),
            }
        }
        let rserver=rng.gen_range(0..candidate_stack.len());
        candidate_stack[rserver]
    }
}

impl ComponentsPattern
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> ComponentsPattern
    {
        let mut component_classes=None;
        //let mut block_pattern=None;
        let mut global_pattern=None;
        match_object_panic!(arg.cv,"Components",value,
			"global_pattern" => global_pattern=Some(new_pattern(MetaPatternBuilderArgument{cv:value,..arg})),
			"component_classes" => component_classes = Some(value.as_array()
				.expect("bad value for component_classes").iter()
				.map(|v|v.as_f64().expect("bad value in component_classes") as usize).collect()),
		);
        let component_classes=component_classes.expect("There were no component_classes");
        //let block_pattern=block_pattern.expect("There were no block_pattern");
        let global_pattern=global_pattern.expect("There were no global_pattern");
        ComponentsPattern{
            component_classes,
            //block_pattern,
            global_pattern,
            components:vec![],//filled at initialize
        }
    }
}


/**
A meta_pattern that returns in order values recieved from a list of values.
```ignore
InmediateSequencePattern{
    sequence: [0,1,2,3,4,5,6,7,8,9],
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct InmediateSequencePattern
{
    sequence: Vec<usize>,
    ///Sequence for each input
    sequences_input: RefCell<Vec<VecDeque<usize>>>,
}

impl MetaPattern<usize, usize>for InmediateSequencePattern
{
    fn initialize(&mut self, source_size:usize, _target_size:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)
    {
        self.sequences_input.replace(vec![VecDeque::from(self.sequence.clone()); source_size]);

    }
    fn get_destination(&self, origin:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)->usize
    {
        self.sequences_input.borrow_mut()[origin].pop_front().unwrap_or(0)
    }
}

impl InmediateSequencePattern
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> InmediateSequencePattern
    {
        let mut sequence=None;
        match_object_panic!(arg.cv,"InmediateSequencePattern",value,
			"sequence" => sequence=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|v|v.as_usize().expect("List should be of usizes")).collect()),
		);
        let sequence = sequence.unwrap();
        InmediateSequencePattern {
            sequence,
            sequences_input: RefCell::new(vec![VecDeque::new()]),
        }
    }
}


/**
For each source, it keeps a state of the last destination used. When applying the meta_pattern, it uses the last destination as the origin for the meta_pattern, and
the destination is saved for the next call to the meta_pattern.
```ignore
ElementComposition{
	meta_pattern: RandomPermutation,
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ElementComposition
{
    ///Pattern to apply.
    pattern: Box<dyn SimplePattern>,
    ///Pending destinations.
    origin_state: RefCell<Vec<usize>>,
}

impl MetaPattern<usize, usize>for ElementComposition
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)
    {
        if source_size!= target_size
        {
            panic!("ElementComposition requires source and target sets to have same size.");
        }
        self.pattern.initialize(source_size,target_size,_topology,_rng);
        self.origin_state.replace((0..source_size).collect());
    }
    fn get_destination(&self, origin:usize, _topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        if origin >= self.origin_state.borrow().len()
        {
            panic!("ElementComposition: origin {} is beyond the source size {}",origin,self.origin_state.borrow().len());
        }
        let index = self.origin_state.borrow_mut()[origin];
        let destination = self.pattern.get_destination(index,_topology,rng);
        self.origin_state.borrow_mut()[origin] = destination;
        destination
    }
}

impl ElementComposition
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> ElementComposition
    {
        let mut pattern = None;
        match_object_panic!(arg.cv,"ElementComposition",value,
			"simple_pattern" | "pattern" => pattern = Some(new_pattern(MetaPatternBuilderArgument{cv:value,..arg})),
		);
        let pattern = pattern.expect("There were no meta_pattern in configuration of ElementComposition.");
        ElementComposition{
            pattern,
            origin_state: RefCell::new(vec![]),
        }
    }
}
/**
 * Pattern which simulates the communications of an all-gather or all-reduce in log p steps, applying the recursive doubling technique.
 * The communications represent a Hypercube.
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RecursiveDistanceHalving
{
    ///Pending destinations.
    origin_state: RefCell<Vec<usize>>,
    ///Map for the different states
    cartesian_data: CartesianData,
    ///Order of the neighbours
    neighbours_order: Option<Vec<Vec<usize>>>,
}

impl MetaPattern<usize, usize>for RecursiveDistanceHalving
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)
    {
        if source_size!= target_size
        {
            panic!("RecursiveDistanceHalving requires source and target sets to have same size.");
        }
        //If the source size is not a power of 2, the meta_pattern will not work.
        if !source_size.is_power_of_two()
        {
            panic!("RecursiveDistanceHalving requires source size to be a power of 2.");
        }
        let pow = source_size.ilog2();
        self.origin_state = RefCell::new(vec![0;source_size]);
        self.cartesian_data = CartesianData::new(&(vec![2; pow as usize]))//(0..pow).map(|i| CartesianData::new(&[source_size/2_usize.pow(i), 2_usize.pow(i)]) ).collect();
    }
    fn get_destination(&self, origin:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)->usize
    {
        if origin >= self.origin_state.borrow().len()
        {
            panic!("RecursiveDistanceHalving: origin {} is beyond the source size {}",origin,self.origin_state.borrow().len());
        }
        let index = self.origin_state.borrow()[origin];
        if index >=self.cartesian_data.sides.len()
        {
            return origin; //No more to do...
        }

        let mut state = self.origin_state.borrow_mut();
        let source_coord = self.cartesian_data.unpack(origin);
        let to_send = if let Some(vectores) = self.neighbours_order.as_ref()
        {
            vectores[state[origin]].clone()
        }else {
            self.cartesian_data.unpack(2_i32.pow(state[origin].try_into().unwrap()) as usize)
        };

        let dest = source_coord.iter().zip(to_send.iter()).map(|(a,b)| a^b).collect::<Vec<usize>>();
        state[origin]+=1;
        self.cartesian_data.pack(&dest)

    }
}

impl RecursiveDistanceHalving
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> RecursiveDistanceHalving
    {
        let mut neighbours_order: Option<Vec<usize>> = None; //Array of vectors which represent the order of the neighbours
        match_object_panic!(arg.cv,"RecursiveDistanceHalving",value,
			"neighbours_order" => neighbours_order = Some(value.as_array().expect("bad value for neighbours_order").iter()
				.map(|n|n.as_usize().unwrap()).collect() ),
		);

        //now each number in the array transform it into an array of binary numbers
        let binary_order = if let Some(n) = neighbours_order
        {
            //get the biggest number
            let max = n.iter().max().unwrap();
            //calculate the number of bits
            let bits = max.ilog2() as usize + 1usize;
            //transform each number into a binary number with the same number of bits
            let bin_n = n.iter().map(|&x| {
                let mut v = vec![0; bits];
                let mut x = x;
                for i in 0..bits
                {
                    v[i] = x%2;
                    x = x/2;
                }
                v
            }).collect();
            Some(bin_n)

        }else{
            None
        };

        RecursiveDistanceHalving{
            origin_state: RefCell::new(vec![]),
            cartesian_data: CartesianData::new(&vec![0;0]),
            neighbours_order: binary_order,
        }
    }
}


/**
 * Pattern to simulate communications in a BinomialTree.
 * Going upwards could be seen as a reduction, and going downwards as a broadcast.
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct BinomialTree
{
    ///How to go through the tree.
    upwards: bool,
    ///Tree embedded into a Hypercube
    cartesian_data: CartesianData,
    ///State indicating the neighbour to send downwards
    state: RefCell<Vec<usize>>,
}

impl MetaPattern<usize, usize>for BinomialTree
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)
    {
        if source_size!= target_size
        {
            panic!("BinomialTree requires source and target sets to have same size.");
        }

        if !source_size.is_power_of_two()
        {
            panic!("BinomialTree requires source size to be a power of 2.");
        }

        let mut tree_order = source_size.ilog2();

        if source_size > 2usize.pow(tree_order)
        {
            tree_order +=1;
        }
        self.cartesian_data = CartesianData::new(&vec![2; tree_order as usize]); // Tree emdebbed into an hypercube
        self.state = RefCell::new(vec![0; source_size]);
    }
    fn get_destination(&self, origin:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)->usize
    {
        if origin >= self.cartesian_data.size
        {
            panic!("BinomialTree: origin {} is beyond the source size {}",origin,self.cartesian_data.size);
        }
        let mut source_coord = self.cartesian_data.unpack(origin);
        let first_one_index = source_coord.iter().enumerate().find(|(_index, &value)| value == 1);

        return if self.upwards
        {
            if origin == 0 {
                0
            } else {
                let first_one_index = first_one_index.unwrap().0;
                let state = self.state.borrow()[origin];
                if state == 1{
                    origin
                }else{
                    self.state.borrow_mut()[origin] = 1;
                    source_coord[first_one_index] = 0;
                    self.cartesian_data.pack(&source_coord)
                }
            }
        }else{
            let first_one_index = if origin == 0{
                self.cartesian_data.sides.len() //log x in base 2... the number of edges in hypercube
            } else{
                first_one_index.unwrap().0
            };
            let son_index = self.state.borrow()[origin];

            if first_one_index > son_index
            {
                self.state.borrow_mut()[origin] += 1;
                origin + 2usize.pow(son_index as u32)
            }else{
                origin // no sons / no more sons to send
            }
        }
    }
}

impl BinomialTree
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> BinomialTree
    {
        let mut upwards = None;
        match_object_panic!(arg.cv,"BinomialTree",value,
			"upwards" => upwards = Some(value.as_bool().expect("bad value for upwards for meta_pattern BinomialTree")),
		);
        let upwards = upwards.expect("There were no upwards in configuration of BinomialTree.");
        BinomialTree{
            upwards,
            cartesian_data: CartesianData::new(&vec![2;2]),
            state: RefCell::new(vec![]),
        }
    }
}




/**
A transparent meta-meta_pattern to help debug other [SimplePattern].

```ignore
Debug{
	meta_pattern: ...,
	check_permutation: true,
}
```
 **/
//TODO: admissible, orders/cycle-finding, suprajective,
#[derive(Debug,Quantifiable)]
pub struct DebugPattern {
    /// The meta_pattern being applied transparently.
    pattern: Box<dyn SimplePattern>,
    /// Whether to consider an error not being a permutation.
    check_permutation: bool,
    /// Size of source cached at initialization.
    source_size: usize,
    /// Size of target cached at initialization.
    target_size: usize,
}

impl MetaPattern<usize, usize>for DebugPattern{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        self.source_size = source_size;
        self.target_size = target_size;
        self.pattern.initialize(source_size,target_size,topology,rng);
        if self.check_permutation {
            if source_size != target_size {
                panic!("cannot be a permutation is source size {} and target size {} do not agree.",source_size,target_size);
            }
            let mut hits = vec![false;target_size];
            for origin in 0..source_size {
                let dst = self.pattern.get_destination(origin,topology,rng);
                if hits[dst] {
                    panic!("Destination {} hit at least twice.",dst);
                }
                hits[dst] = true;
            }
        }
    }
    fn get_destination(&self, origin:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        if origin >= self.source_size {
            panic!("Received an origin {origin} beyond source size {size}",size=self.source_size);
        }
        let dst = self.pattern.get_destination(origin,topology,rng);
        if dst >= self.target_size {
            panic!("The destination {dst} is beyond the target size {size}",size=self.target_size);
        }
        dst
    }
}

impl DebugPattern{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> DebugPattern{
        let mut pattern = None;
        let mut check_permutation = false;
        match_object_panic!(arg.cv,"Debug",value,
			"simple_pattern" | "pattern" => pattern = Some(new_pattern(MetaPatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"check_permutation" => check_permutation = value.as_bool().expect("bad value for check_permutation"),
		);
        let pattern = pattern.expect("Missing meta_pattern in configuration of Debug.");
        DebugPattern{
            pattern,
            check_permutation,
            source_size:0,
            target_size:0,
        }
    }
}

// /**
// Neighbours meta_pattern that selects the neighbours of a node in a space.
// ```ignore
//
// NearestNeighbours{ //Iter the neighbours of a node in each dimension. No wrap-around
//     sides: [3,3],
// }
//
// ManhattanNeighbours{ //Iter the neighbours inside a manhattan distance. No wrap-around
//     sides: [3,3],
//     distance: 1,
// }
//
// KingNeighbours{ //Iter the neighbours inside a chessboard distance. No wrap-around
//     sides: [3,3],
//     distance: 1,
// }
// **/
// #[derive(Quantifiable)]
// #[derive(Debug)]
// pub struct EncapsulatedPattern {}
//
// impl EncapsulatedPattern {
//     pub(crate) fn new(pattern: String, arg:MetaPatternBuilderArgument) -> Box<dyn SimplePattern> {
//         let pattern_cv = match pattern.as_str(){
//             "NearestNeighbours" =>{
//                 let mut sides = None;
//                 match_object_panic!(arg.cv,"NearestNeighbours",value,
// 					"sides" => sides = Some(value.as_array().expect("bad value for sides").iter()
// 						.map(|v|v.as_usize().expect("bad value in sides")).collect()),
// 				);
//                 let sides = sides.expect("There were no sides in configuration of Stencil.");
//                 Some(get_nearest_neighbours_pattern(sides))
//             },
//             "ManhattanNeighbours" =>{
//                 let mut distance = None;
//                 let mut sides = None;
//                 match_object_panic!(arg.cv,"ManhattanNeighbours",value,
//                     "sides" => sides = Some(value.as_array().expect("bad value for sides").iter()
//                         .map(|v|v.as_usize().expect("bad value in sides")).collect()),
//                     "distance" => distance = Some(value.as_usize().expect("bad value for distance")),
//                 );
//                 let distance = distance.expect("There were no distance in configuration of ManhattanNeighbours.");
//                 let sides = sides.expect("There were no sides in configuration of ManhattanNeighbours.");
//                 Some(get_manhattan_neighbours_pattern(&sides, distance))
//             },
//             "KingNeighbours" =>{
//                 let mut distance = None;
//                 let mut sides = None;
//                 match_object_panic!(arg.cv,"KingNeighbours",value,
//                     "sides" => sides = Some(value.as_array().expect("bad value for sides").iter()
//                         .map(|v|v.as_usize().expect("bad value in sides")).collect()),
//                     "distance" => distance = Some(value.as_usize().expect("bad value for distance")),
//                 );
//                 let distance = distance.expect("There were no distance in configuration of KingNeighbours.");
//                 let sides = sides.expect("There were no sides in configuration of KingNeighbours.");
//                 Some(get_king_neighbours_pattern(&sides, distance))
//             },
//             _ => panic!("Pattern {} not found.",pattern),
//         };
//         new_pattern(MetaPatternBuilderArgument{cv:&pattern_cv.unwrap(),..arg})
//     }
// }
//
// pub fn get_nearest_neighbours_pattern(sides: Vec<usize>) -> ConfigurationValue
// {
//     get_manhattan_neighbours_pattern(&sides, 1)
// }
//
// pub fn get_neighbours_pattern(sides: Vec<usize>, transforms: Vec<Vec<i32>>) -> ConfigurationValue
// {
//     let space_cv = ConfigurationValue::Array(sides.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>());
//
//     let mut transforms_cv = vec![];
//     for i in transforms
//     {
//         transforms_cv.push(
//             ConfigurationValue::Object("AddVector".to_string(), vec![
//                 ("sides".to_string(), space_cv.clone()),
//                 ("shift".to_string(), ConfigurationValue::Array(i.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<ConfigurationValue>>())),
//                 ("modulo".to_string(), ConfigurationValue::False),
//             ]),
//         );
//     }
//
//     ConfigurationValue::Object( "DestinationSets".to_string(), vec![
//         ("patterns".to_string(), ConfigurationValue::Array(transforms_cv)),
//         ("exclude_self_references".to_string(), ConfigurationValue::True),
//     ])
// }
//
// pub fn get_king_neighbours_pattern(sides: &Vec<usize>, chessboard_distance: usize) -> ConfigurationValue
// {
//     let transforms = get_vectors_in_king_distance(sides, chessboard_distance);
//     get_neighbours_pattern(sides.clone(), transforms)
// }
//
// /// sides are the sides of the space, chessboard_distance is the distance to be considered
// pub fn get_vectors_in_king_distance(sides: &[usize], chessboard_distance: usize) -> Vec<Vec<i32>>
// {
//     let vectors = get_vectors_in_king_distance_aux(sides, chessboard_distance);
//     let mut vectors:Vec<Vec<i32>> = vectors.into_iter().filter(|e| !e.iter().all(|&i| i == 0)).collect(); //remove all 0s
//     vectors.sort();
//     vectors
// }
//
// pub fn get_vectors_in_king_distance_aux(sides: &[usize], chessboard_distance: usize) -> Vec<Vec<i32>>
// {
//     let total_dist = chessboard_distance as i32;
//     if chessboard_distance == 0
//     {
//         vec![vec![0;sides.len()]]
//     }else if sides.len() == 1
//     {
//         (-total_dist..=total_dist).map(|i| vec![i] ).collect()
//     } else {
//         let mut vectors = vec![];
//         let vec = get_vectors_in_king_distance_aux(&sides[1..], chessboard_distance);
//
//         let vec_1 = extend_vectors(vec.clone(), 0);
//         vectors.extend(vec_1);
//
//         for dist in 1..=total_dist
//         {
//             let vec_1 = extend_vectors(vec.clone(), dist);
//             vectors.extend(vec_1);
//
//             let vec_2 = extend_vectors(vec.clone(), -dist);
//             vectors.extend(vec_2);
//         }
//         vectors
//     }
// }
//
// pub fn get_manhattan_neighbours_pattern(sides: &Vec<usize>, manhattan_distance: usize) -> ConfigurationValue
// {
//     let transforms = get_vectors_in_manhattan_distance(sides, manhattan_distance);
//     get_neighbours_pattern(sides.clone(), transforms)
// }
//
// /// sides are the sides of the space, manhattan_distance is the distance to be considered
// pub fn get_vectors_in_manhattan_distance(sides: &Vec<usize>, manhattan_distance: usize) -> Vec<Vec<i32>>
// {
//     let vectors = get_vectors_in_manhattan_distance_aux(sides, manhattan_distance);
//     let mut vectors:Vec<Vec<i32>> = vectors.into_iter().filter(|e| !e.iter().all(|&i| i == 0)).collect(); //remove all 0s
//     vectors.sort();
//     vectors
// }
//
//
// /// sides are the sides of the space, manhattan_distance is the distance to be considered
// pub fn get_vectors_in_manhattan_distance_aux(sides: &[usize], manhattan_distance: usize) -> Vec<Vec<i32>>
// {
//     let total_dist = manhattan_distance as i32;
//     if manhattan_distance == 0
//     {
//         vec![vec![0;sides.len()]]
//     } else if sides.len() == 1{
//         (-total_dist..=total_dist).map(|i| vec![i] ).collect()
//     } else {
//         let mut vectors = vec![];
//         for dist in 0..=total_dist
//         {
//             let vec = get_vectors_in_manhattan_distance_aux(&sides[1..], (total_dist - dist) as usize);
//             let vec_1 = extend_vectors(vec.clone(), dist);
//             vectors.extend(vec_1);
//             if dist != 0{
//                 let vec_2 = extend_vectors(vec.clone(), -dist);
//                 vectors.extend(vec_2);
//             }
//         }
//         vectors
//     }
// }
//
//
// pub fn extend_vectors(vectors: Vec<Vec<i32>>, value: i32) -> Vec<Vec<i32>>
// {
//     let mut new_vectors = vec![];
//     for mut vector in vectors
//     {
//         vector.push(value);
//         new_vectors.push(vector);
//     }
//     new_vectors
// }


pub fn get_switch_pattern(index_pattern: ConfigurationValue, patterns: Vec<ConfigurationValue>) -> ConfigurationValue{
    ConfigurationValue::Object("Switch".to_string(), vec![
        ("indexing".to_string(), index_pattern),
        ("patterns".to_string(), ConfigurationValue::Array(patterns)),
    ])
}

pub fn get_candidates_selection(pattern: ConfigurationValue, pattern_destination_size: usize) -> ConfigurationValue{
    ConfigurationValue::Object("CandidatesSelection".to_string(), vec![
        ("simple_pattern".to_string(), pattern),
        ("pattern_destination_size".to_string(), ConfigurationValue::Number(pattern_destination_size as f64)),
    ])
}

pub fn get_cartesian_transform(sides: Vec<usize>, shift: Option<Vec<usize>>, patterns: Option<Vec<ConfigurationValue>>) -> ConfigurationValue{
    let mut config = vec![
        ("sides".to_string(), ConfigurationValue::Array(sides.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>())),
    ];
    if let Some(shift) = shift{
        config.push(("shift".to_string(), ConfigurationValue::Array(shift.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>())));
    }
    if let Some(patterns) = patterns{
        config.push(("patterns".to_string(), ConfigurationValue::Array(patterns)));
    }
    ConfigurationValue::Object("CartesianTransform".to_string(), config)
}

pub fn get_hotspot_destination(selected_destinations: Vec<usize>) -> ConfigurationValue{
    ConfigurationValue::Object("Hotspots".to_string(), vec![
        ("destinations".to_string(), ConfigurationValue::Array(selected_destinations.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>()), )
    ])
}


/**
FOR ALEX, NO MASTER
 **/
//TODO: admissible, orders/cycle-finding, suprajective,
#[derive(Debug,Quantifiable)]
pub struct MiDebugPattern {
    /// The meta_pattern being applied transparently.
    pattern: Vec<Box<dyn SimplePattern>>,
    /// Whether to consider an error not being a permutation.
    check_permutation: bool,
    /// Whether to consider an error not being an injection.
    check_injective: bool,
    /// Size of source cached at initialization.
    source_size: Vec<usize>,
    /// Size of target cached at initialization.
    target_size: usize,
}

impl MetaPattern<usize, usize>for MiDebugPattern {
    fn initialize(&mut self, _source_size:usize, _target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        // self.source_size = source_size;
        // self.target_size = target_size;
        for (index, pattern) in self.pattern.iter_mut().enumerate() {
            pattern.initialize(self.source_size[index], self.target_size, topology,rng);
        }

        if self.check_injective{

            if self.source_size.iter().sum::<usize>() > self.target_size{
                panic!("cannot be injective if source size {} is more than target size {}",self.source_size.iter().sum::<usize>(),self.target_size);
            }
            let mut hits = vec![-1;self.target_size];
            for (index, size) in self.source_size.iter().enumerate() {

                for origin_local in 0..*size {
                    let dst = self.pattern[index].get_destination(origin_local,topology,rng);
                    if hits[dst] != -1 {
                        panic!("Destination {} hit by origin {}, now by {}, in meta_pattern: {}",dst,hits[dst],origin_local, index);
                    }
                    hits[dst] = origin_local as isize;
                }

            }
            println!("Check injective patterns passed.");
            println!("There were the following number of sources: {:?} ({}), and the following number of destinations: {}",self.source_size,self.source_size.iter().sum::<usize>(),self.target_size);
            println!("There are {} free destinations, and {} servers hits. The free destinations are: {:?}",hits.iter().filter(|x|**x==-1).count(),hits.iter().filter(|x|**x!=-1).count(),hits.iter().enumerate().filter(|(_,x)|**x==-1).map(|(i,_)|i).collect::<Vec<usize>>());

        }
        // if self.check_permutation {
        // 	if self.source_size != self.target_size {
        // 		panic!("cannot be a permutation is source size {} and target size {} do not agree.",self.source_size,self.target_size);
        // 	}
        // 	let mut hits = vec![false;self.target_size];
        // 	for origin in 0..self.source_size {
        // 		let dst = self.meta_pattern.get_destination(origin,topology,rng);
        // 		if hits[dst] {
        // 			panic!("Destination {} hit at least twice.",dst);
        // 		}
        // 		hits[dst] = true;
        // 	}
        // }
        panic!("This is just a check.")
    }
    fn get_destination(&self, _origin:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)->usize
    {
        0
        // if origin >= self.source_size {
        // 	panic!("Received an origin {origin} beyond source size {size}",size=self.source_size);
        // }
        // let dst = self.meta_pattern.get_destination(origin,topology,rng);
        // if dst >= self.target_size {
        // 	panic!("The destination {dst} is beyond the target size {size}",size=self.target_size);
        // }
        // dst
    }
}

impl MiDebugPattern {
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> MiDebugPattern {
        let mut pattern = None;
        let mut check_permutation = false;
        let mut check_injective = false;
        let mut source_size = None;
        let mut target_size = None;
        match_object_panic!(arg.cv,"Debug",value,
			"patterns" => pattern = Some(value.as_array().expect("bad value for meta_pattern").iter()
				.map(|pcv|new_pattern(MetaPatternBuilderArgument{cv:pcv,..arg})).collect()),
			"check_permutation" => check_permutation = value.as_bool().expect("bad value for check_permutation"),
			"source_size" => source_size = Some(value.as_array().expect("bad value for source_size").iter()
				.map(|v|v.as_usize().expect("bad value in source_size")).collect()),
			"target_size" => target_size = Some(value.as_usize().expect("bad value for target_size")),
			"check_injective" => check_injective = value.as_bool().expect("bad value for check_injective"),
		);
        let pattern = pattern.expect("Missing meta_pattern in configuration of Debug.");
        let source_size = source_size.expect("Missing source_size in configuration of Debug.");
        let target_size = target_size.expect("Missing target_size in configuration of Debug.");
        MiDebugPattern {
            pattern,
            check_permutation,
            check_injective,
            source_size,
            target_size,
        }
    }
}