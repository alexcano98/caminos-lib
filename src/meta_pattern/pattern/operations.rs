use crate::meta_pattern::pattern::Pattern;
use crate::meta_pattern::GeneralPattern;
use std::cell::{RefCell};
use ::rand::{rngs::StdRng};
use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::topology::{Topology};
use crate::{match_object_panic};
use rand::{RngCore, SeedableRng};
use crate::meta_pattern::{new_pattern, MetaPatternBuilderArgument};


///A meta_pattern given by blocks. The elements are divided by blocks of size `block_size`. The `global_pattern` is used to describe the communication among different blocks and the `block_pattern` to describe the communication inside a block.
///Seen as a graph, this is the Kronecker product of the block graph with the global graph.
///Thus the origin a position `i` in the block `j` will select the destination at position `b(i)` in the block `g(j)`, where `b(i)` is the destination via the `block_pattern` and `g(j)` is the destination via the `global_pattern`.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ProductPattern
{
    block_size: usize,
    block_pattern: Box<dyn Pattern>,
    global_pattern: Box<dyn Pattern>,
}

impl GeneralPattern<usize, usize>for ProductPattern
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        if source_size!=target_size
        {
            unimplemented!("Different sizes are not yet implemented for ProductPattern");
        }
        let topology = topology.expect("ProductPattern requires a topology");

        self.block_pattern.initialize(self.block_size,self.block_size,Some(topology),rng);
        let global_size=source_size/self.block_size;
        self.global_pattern.initialize(global_size,global_size,Some(topology),rng);
    }
    fn get_destination(&self, origin:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        let local=origin % self.block_size;
        let global=origin / self.block_size;
        let local_dest=self.block_pattern.get_destination(local,topology,rng);
        let global_dest=self.global_pattern.get_destination(global,topology,rng);
        global_dest*self.block_size+local_dest
    }
}

impl ProductPattern
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> ProductPattern
    {
        let mut block_size=None;
        let mut block_pattern=None;
        let mut global_pattern=None;
        match_object_panic!(arg.cv,"Product",value,
			"block_pattern" => block_pattern=Some(new_pattern(MetaPatternBuilderArgument{cv:value,..arg})),
			"global_pattern" => global_pattern=Some(new_pattern(MetaPatternBuilderArgument{cv:value,..arg})),
			"block_size" => block_size=Some(value.as_f64().expect("bad value for block_size") as usize),
		);
        let block_size=block_size.expect("There were no block_size");
        let block_pattern=block_pattern.expect("There were no block_pattern");
        let global_pattern=global_pattern.expect("There were no global_pattern");
        ProductPattern{
            block_size,
            block_pattern,
            global_pattern,
        }
    }
}

/**
The meta_pattern resulting of composing a list of patterns.
`destination=patterns[len-1]( patterns[len-2] ( ... (patterns[1] ( patterns[0]( origin ) )) ) )`.
The intermediate sizes along the composition can be stated by `middle_sizes`, otherwise they are set equal to the `target_size` of the whole.
Thus in a composition of two patterns in which the midddle size is `x`and not equal to `target_size`, it should be set `middle_sizes=[x]`.
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Composition
{
    patterns: Vec<Box<dyn Pattern>>,
    middle_sizes: Vec<usize>,
}

impl GeneralPattern<usize, usize>for Composition
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        for (index,pattern) in self.patterns.iter_mut().enumerate()
        {
            let current_source = if index==0 { source_size } else { *self.middle_sizes.get(index-1).unwrap_or(&target_size) };
            let current_target = *self.middle_sizes.get(index).unwrap_or(&target_size);
            pattern.initialize(current_source,current_target,topology,rng);
        }
    }
    fn get_destination(&self, origin:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        let mut destination=origin;
        for pattern in self.patterns.iter()
        {
            destination=pattern.get_destination(destination,topology,rng);
        }
        destination
    }
}

impl Composition
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> Composition
    {
        let mut patterns=None;
        let mut middle_sizes=None;
        match_object_panic!(arg.cv,"Composition",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(MetaPatternBuilderArgument{cv:pcv,..arg})).collect()),
			"middle_sizes" => middle_sizes = Some(value.as_array().expect("bad value for middle_sizes").iter()
				.map(|v|v.as_usize().expect("bad value for middle_sizes")).collect()),
		);
        let patterns=patterns.expect("There were no patterns");
        let middle_sizes = middle_sizes.unwrap_or_else(||vec![]);
        Composition{
            patterns,
            middle_sizes,
        }
    }
}


/**
 For a source, it sums the result of applying several patterns.
 For instance, the destination of a server a would be: dest(a) = p1(a) + p2(a) + p3(a).
 middle_sizes indicates the size of the intermediate patters.

Sum{ //A vector of 2's
	patterns:[
		CandidatesSelection{
				meta_pattern: Identity,
				pattern_destination_size: 2048,
		},
		CandidatesSelection{
				meta_pattern: Identity,
				pattern_destination_size: 2048,
		},
	],
	middle_sizes: [2,2],
},
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Sum
{
    patterns: Vec<Box<dyn Pattern>>,
    middle_sizes: Vec<usize>,
    target_size: Option<usize>,
}

impl GeneralPattern<usize, usize>for Sum
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        for (index,pattern) in self.patterns.iter_mut().enumerate()
        {
            // let current_source = if index==0 { source_size } else { *self.middle_sizes.get(index-1).unwrap_or(&target_size) };
            let current_target = *self.middle_sizes.get(index).unwrap_or(&target_size);
            pattern.initialize(source_size,current_target,topology,rng);
        }
        self.target_size = Some(target_size);
    }
    fn get_destination(&self, origin:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        let target_size = self.target_size.unwrap();
        let mut destination=0;
        for pattern in self.patterns.iter()
        {
            let next_destination = pattern.get_destination(origin,topology,rng);
            destination+=next_destination;
        }
        if destination>=target_size
        {
            panic!("Sum pattern overflowed the target size.")
        }
        destination
    }
}

impl Sum
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> Sum
    {
        let mut patterns=None;
        let mut middle_sizes=None;
        match_object_panic!(arg.cv,"Sum",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(MetaPatternBuilderArgument{cv:pcv,..arg})).collect()),
			"middle_sizes" => middle_sizes = Some(value.as_array().expect("bad value for middle_sizes").iter()
				.map(|v|v.as_usize().expect("bad value for middle_sizes")).collect()),
		);
        let patterns=patterns.expect("There were no patterns");
        let middle_sizes = middle_sizes.unwrap_or_else(||vec![]);
        Sum{
            patterns,
            middle_sizes,
            target_size: None,
        }
    }
}


///The meta_pattern resulting of composing a meta_pattern with itself a number of times..
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Pow
{
    pattern: Box<dyn Pattern>,
    exponent: usize,
}

impl GeneralPattern<usize, usize>for Pow
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        self.pattern.initialize(source_size,target_size,topology,rng);
    }
    fn get_destination(&self, origin:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        let mut destination=origin;
        for _ in 0..self.exponent
        {
            destination=self.pattern.get_destination(destination,topology,rng);
        }
        destination
    }
}

impl Pow
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> Pow
    {
        let mut pattern=None;
        let mut exponent=None;
        match_object_panic!(arg.cv,"Pow",value,
			"pattern"  => pattern=Some(new_pattern(MetaPatternBuilderArgument{cv:value,..arg})),
			"exponent" => exponent=Some(value.as_f64().expect("bad value for exponent") as usize),
		);
        let pattern=pattern.expect("There were no meta_pattern");
        let exponent=exponent.expect("There were no exponent");
        Pow{
            pattern,
            exponent,
        }
    }
}


/**
Use a list of patterns in a round robin fashion, for each source.

RoundRobin{ // Alternate between three random permutations
	patterns: [RandomPermutation, RandomPermutation, RandomPermutation],
}
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RoundRobin
{
    ///The patterns in the pool to be selected.
    patterns: Vec<Box<dyn Pattern>>,
    /// Vec meta_pattern origin
    index: RefCell<Vec<usize>>,
}

impl GeneralPattern<usize, usize>for RoundRobin
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        if self.patterns.is_empty()
        {
            panic!("RoundRobin requires at least one pattern (and 2 to be sensible).");
        }
        for pat in self.patterns.iter_mut()
        {
            pat.initialize(source_size,target_size,topology,rng);
        }
        self.index.replace(vec![0;source_size]);
    }
    fn get_destination(&self, origin:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        let mut indexes = self.index.borrow_mut();
        let pattern_index = indexes[origin];
        indexes[origin] = (pattern_index+1) % self.patterns.len();
        self.patterns[pattern_index].get_destination(origin,topology,rng)
    }
}

impl RoundRobin
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> RoundRobin
    {
        let mut patterns=None;
        match_object_panic!(arg.cv,"RoundRobin",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(MetaPatternBuilderArgument{cv:pcv,..arg})).collect()),
		);
        let patterns=patterns.expect("There were no patterns");
        RoundRobin{
            patterns,
            index: RefCell::new(Vec::new()),
        }
    }
}



/**
For each server, it keeps a shuffled list of destinations to which send.
It removes destinations which point to self.

```ignore
DestinationSets{
	patterns: [RandomPermutation, RandomPermutation, RandomPermutation], //3 random destinations
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct DestinationSets
{
    ///Patterns to get the set of destinations
    patterns: Vec<Box<dyn Pattern>>,
    ///Set of destinations.
    destination_set: Vec<Vec<usize>>,
    ///Exclude self references
    exclude_self_references: bool,
    ///Indexes
    index: RefCell<Vec<usize>>,
}

impl GeneralPattern<usize, usize>for DestinationSets
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        self.destination_set = vec![vec![]; source_size];
        self.index.replace(vec![0;source_size]);
        for (_index,pattern) in self.patterns.iter_mut().enumerate()
        {
            pattern.initialize(source_size,target_size,topology,rng);
            for source in 0..source_size
            {
                let destination = pattern.get_destination(source,topology,rng);
                if !self.exclude_self_references || source!=destination
                {
                    self.destination_set[source].push(destination);
                }
            }
        }
    }
    fn get_destination(&self, origin:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)->usize
    {
        let mut indexes = self.index.borrow_mut();
        let pattern_index = indexes[origin];
        let destination = self.destination_set[origin][pattern_index];
        indexes[origin] = (indexes[origin]+1) % self.destination_set[origin].len();
        destination
    }
}

impl DestinationSets
{
    pub fn new(arg:MetaPatternBuilderArgument) -> DestinationSets
    {
        let mut patterns=None;
        let mut exclude_self_references = false;
        match_object_panic!(arg.cv,"DestinationSets",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(MetaPatternBuilderArgument{cv:pcv,..arg})).collect()),
            "exclude_self_references" => exclude_self_references = value.as_bool().expect("bad value for exclude_self_references"),
		);
        let patterns:Vec<Box<dyn Pattern>>=patterns.expect("There were no patterns");

        DestinationSets{
            patterns,
            destination_set:vec![],//to be filled in initialization
            exclude_self_references,
            index: RefCell::new(vec![]),
        }
    }
}

/**
```
	Uses the inverse of the meta_pattern specified.
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Inverse
{
    ///Pattern to apply.
    pattern: Box<dyn Pattern>,
    ///Destination
    inverse_values: Vec<Option<usize>>,
    ///default destination
    default_destination: Option<usize>,
}

impl GeneralPattern<usize, usize>for Inverse
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)
    {
        // if source_size!= target_size
        // {
        // 	panic!("Inverse requires source and target sets to have same size.");
        // }
        self.pattern.initialize(source_size,target_size,_topology,_rng);
        let mut source = vec![None; source_size];
        for i in 0..source_size
        {
            let destination = self.pattern.get_destination(i,_topology,_rng);
            if let Some(_) = source[destination]
            {
                panic!("Inverse: destination {} is already used by origin {}.",destination,source[destination].unwrap());
            }
            source[destination] = Some(i);
        }
        self.inverse_values = source;
    }
    fn get_destination(&self, origin:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)->usize
    {
        if origin >= self.inverse_values.len()
        {
            panic!("Inverse: origin {} is beyond the source size {}",origin,self.inverse_values.len());
        }
        if let Some(destination) = self.inverse_values[origin]
        {
            destination
        }
        else
        {
            self.default_destination.expect(&*("Inverse: origin ".to_owned() + &*origin.to_string() + " has no destination and there is no default destination."))
        }
    }
}

impl Inverse
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> Inverse
    {
        let mut pattern = None;
        let mut default_destination = None;
        match_object_panic!(arg.cv,"Inverse",value,
			"pattern"  => pattern = Some(new_pattern(MetaPatternBuilderArgument{cv:value,..arg})),
			"default_destination" => default_destination = Some(value.as_usize().expect("bad value for default_destination")),
		);
        let pattern = pattern.expect("There were no meta_pattern in configuration of Inverse.");
        Inverse{
            pattern,
            inverse_values: vec![],
            default_destination,
        }
    }
}

/**

Select a region of tasks to execute a meta_pattern. The size of the application using the meta_pattern is 64.
```ignore
	SubApp{
		subtasks: 8,
		selection_pattern: CartesianEmbedding{
			source_sides: [1,8],
			destination_sides: [8,8],
		},
		subapp_pattern: CartesianTransform{
			sides: [8, 8],
			shift: [0, 1],
		},
		others_pattern: RandomPermutation,
	}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct SubApp
{
    subtasks: usize,
    selection_pattern: Box<dyn Pattern>,
    subapp_pattern: Box<dyn Pattern>,
    others_pattern: Box<dyn Pattern>,
    selected_vec: Vec<usize>,
}

impl GeneralPattern<usize, usize>for SubApp
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)
    {

        if self.subtasks > source_size
        {
            panic!("SubApp: subtasks {} is greater than source size {}.",self.subtasks,source_size);
        }

        self.selection_pattern.initialize( self.subtasks, target_size, _topology, _rng);
        self.subapp_pattern.initialize(source_size,target_size,_topology,_rng);
        self.others_pattern.initialize(source_size,target_size,_topology,_rng);

        let mut source = vec![0; source_size];
        (0..self.subtasks).for_each(|i| {
            let destination = self.selection_pattern.get_destination(i,_topology,_rng);
            source[destination] = 1;
        });
        self.selected_vec = source;

    }
    fn get_destination(&self, origin:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)->usize
    {
        if self.selected_vec.len() <= origin
        {
            panic!("SubApp: origin {} is beyond the source size {}",origin,self.selected_vec.len());
        }

        if self.selected_vec[origin] == 1
        {
            self.subapp_pattern.get_destination(origin,_topology,_rng)
        }
        else
        {
            self.others_pattern.get_destination(origin,_topology,_rng)
        }

    }
}

impl SubApp
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> SubApp
    {
        let mut subtasks = None;
        let mut selection_pattern = None;
        let mut subapp_pattern = None;
        let mut others_pattern = None;
        match_object_panic!(arg.cv,"SubApp",value,
			"subtasks" => subtasks = Some(value.as_usize().expect("bad value for total_subsize")),
			"selection_pattern" => selection_pattern = Some(new_pattern(MetaPatternBuilderArgument{cv:value,plugs:arg.plugs})), //map of the application over the machine
			"subapp_pattern" => subapp_pattern = Some(new_pattern(MetaPatternBuilderArgument{cv:value,plugs:arg.plugs})), //traffic of the application
			"others_pattern" => others_pattern = Some(new_pattern(MetaPatternBuilderArgument{cv:value,plugs:arg.plugs})), //traffic of the machine
		);

        let subtasks = subtasks.expect("There were no tasks in configuration of SubApp.");
        let subapp_pattern = subapp_pattern.expect("There were no subapp_pattern in configuration of SubApp.");
        let selection_pattern = selection_pattern.expect("There were no selection_pattern in configuration of SubApp.");
        let others_pattern = others_pattern.expect("There were no others_pattern in configuration of SubApp.");

        SubApp{
            subtasks,
            subapp_pattern,
            selection_pattern,
            others_pattern,
            selected_vec: vec![],
        }

    }
}


/**
Boolean function which puts a 1 if the meta_pattern contains the server, and 0 otherwise.
```ignore
CandidatesSelection{
	meta_pattern: Hotspots{selected_destinations: [0]}, //1 if the server is 0, 0 otherwise
	pattern_destination_size: 1,
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CandidatesSelection
{
    ///Pattern to apply.
    selected: Option<Vec<usize>>,
    ///Pattern to apply.
    pattern: Box<dyn Pattern>,
    ///Pattern destination size.
    pattern_destination_size: usize,
}

impl GeneralPattern<usize, usize>for CandidatesSelection
{
    fn initialize(&mut self, source_size:usize, _target_size:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)
    {
        // if target_size != 2
        // {
        // 	panic!("CandidatesSelection requires target size to be 2.");
        // }
        self.pattern.initialize(source_size, self.pattern_destination_size, _topology, _rng);
        let mut selection = vec![0;source_size];
        for i in 0..source_size
        {
            selection[self.pattern.get_destination(i,_topology,_rng)] = 1;
        }
        self.selected = Some(selection);
    }
    fn get_destination(&self, origin:usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng)->usize
    {
        if origin >= self.selected.as_ref().unwrap().len()
        {
            panic!("CandidatesSelection: origin {} is beyond the source size {}",origin,self.selected.as_ref().unwrap().len());
        }
        self.selected.as_ref().unwrap()[origin]
    }
}

impl CandidatesSelection
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> CandidatesSelection
    {
        let mut pattern = None;
        let mut pattern_destination_size = None;
        match_object_panic!(arg.cv,"CandidatesSelection",value,
			"pattern"  => pattern = Some(new_pattern(MetaPatternBuilderArgument{cv:value,..arg})),
			"pattern_destination_size" => pattern_destination_size = Some(value.as_usize().expect("bad value for pattern_destination_size")),
		);
        let pattern = pattern.expect("There were no meta_pattern in configuration of CandidatesSelection.");
        let pattern_destination_size = pattern_destination_size.expect("There were no pattern_destination_size in configuration of CandidatesSelection.");
        CandidatesSelection{
            selected: None,
            pattern,
            pattern_destination_size,
        }
    }
}

/// Partition the nodes in independent regions, each with its own meta_pattern. Source and target sizes must be equal.
/// ```ignore
/// IndependentRegions{
/// 	// An array with the patterns for each region.
/// 	patterns: [Uniform, Hotspots{destinations:[0]}],
/// 	// An array with the size of each region. They must add up to the total size.
/// 	sizes: [100, 50],
/// 	// Alternatively, use relative_sizes. the meta_pattern will be initialized with sizes proportional to these.
/// 	// You must use exactly one of either `sizes` or `relative_sizes`.
/// 	// relative_sizes: [88, 11],
/// }
/// ```
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct IndependentRegions
{
    /// The actual size of each region. An empty vector if not given nor initialized.
    /// If not empty it must sum up to the total size and have as many elements as the `patterns` field.
    sizes: Vec<usize>,
    /// The meta_pattern to be employed in each region.
    patterns: Vec<Box<dyn Pattern>>,
    /// If not empty, it is used to build the actual `sizes`.
    relative_sizes: Vec<f64>,
}

/**
Build an integer vector with elements proportional to the given `weights` and with a total `target_sum`.
Based on <https://stackoverflow.com/questions/16226991/allocate-an-array-of-integers-proportionally-compensating-for-rounding-errors>
 **/
pub fn proportional_vec_with_sum(weights:&Vec<f64>, target_sum:usize) -> Vec<usize>
{
    let mut result : Vec<usize> = Vec::with_capacity(weights.len());
    let mut total_weight : f64 = weights.iter().sum();
    let mut target_sum : f64 = target_sum as f64;
    for &w in weights
    {
        let rounded : f64 = ( w*target_sum/total_weight ).round();
        result.push(rounded as usize);
        total_weight -= w;
        target_sum -= rounded;
    }
    result
}

impl GeneralPattern<usize, usize>for IndependentRegions
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        assert_eq!(source_size, target_size, "source_size and target_size must be equal in IndependentRegions.");
        if !self.relative_sizes.is_empty()
        {
            assert!(self.sizes.is_empty(),"Cannot set both sizes and relative_sizes in IndependentRegions.");
            // Just doing this do not work. Consider [37,37,74] for 150, which gives [38,38,75].
            //let relative_total: f64 = self.relative_sizes.iter().sum();
            //let scale : f64 = source_size as f64 / relative_total;
            //let expected_sizes : Vec<f64> = self.relative_sizes.iter().map(|x|x*scale).collect();
            //self.sizes = expected_sizes.iter().map(|x|x.round() as usize).collect();
            //TODO: Is this guaranteed to sum correctly??
            self.sizes = proportional_vec_with_sum(&self.relative_sizes,source_size);
        }
        assert_eq!(self.sizes.iter().sum::<usize>(), source_size, "IndependentRegions sizes {:?} do not add up to the source_size {}", self.sizes, source_size);
        for region_index in 0..self.patterns.len()
        {
            let size = self.sizes[region_index];
            self.patterns[region_index].initialize(size,size,topology,rng);
        }
    }
    fn get_destination(&self, mut origin:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        let mut region_index = 0;
        let mut region_offset = 0;
        while origin >= self.sizes[region_index]
        {
            origin -= self.sizes[region_index];
            region_offset += self.sizes[region_index];
            region_index += 1;
        }
        let destination = self.patterns[region_index].get_destination(origin,topology,rng);
        destination + region_offset
    }
}

impl IndependentRegions
{
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> IndependentRegions
    {
        let mut patterns : Option<Vec<_>> = None;
        let mut sizes = None;
        let mut relative_sizes = None;
        match_object_panic!(arg.cv,"IndependentRegions",value,
			"patterns" => patterns = Some(value.as_array().expect("bad value for patterns").iter()
				.map(|v|new_pattern(MetaPatternBuilderArgument{cv:v,..arg})).collect()),
			"sizes" => sizes = Some(value.as_array()
				.expect("bad value for sizes").iter()
				.map(|v|v.as_f64().expect("bad value in sizes") as usize).collect()),
			"relative_sizes" => relative_sizes = Some(value.as_array()
				.expect("bad value for relative_sizes").iter()
				.map(|v|v.as_f64().expect("bad value in relative_sizes")).collect()),
		);
        let patterns = patterns.expect("There was no patterns.");
        assert!( matches!(sizes,None) || matches!(relative_sizes,None), "Cannot set both sizes and relative_sizes." );
        assert!( !matches!(sizes,None) || !matches!(relative_sizes,None), "Must set one of sizes or relative_sizes." );
        let sizes = sizes.unwrap_or_else(||Vec::new());
        let relative_sizes = relative_sizes.unwrap_or_else(||Vec::new());
        assert_eq!(patterns.len(), sizes.len().max(relative_sizes.len()), "Different number of entries in IndependentRegions.");
        IndependentRegions{
            patterns,
            sizes,
            relative_sizes,
        }
    }
}

/**
Use a `indexing` meta_pattern to select among several possible patterns from the input to the output.
The `indexing` is initialized as a meta_pattern from the input size to the number of `patterns`.
This is a Switch meta_pattern, not a [Router] of packets.

This example keeps the even fixed and send odd input randomly. These odd input select even or odd indistinctly.
```ignore
Switch{
	indexing: LinearTansform{
		source_size: [2, 10],
		target_size: [2],
		matrix: [
			[1, 0],
		],
	},
	patterns: [
		Identity,
		Uniform,
	],
	seed: 1234, //root to define a sequence of seeds to use in the initialization of the patterns.
}
```

In this example the nodes at `(0,y)` are sent to a `(y,0,0)` row.
And the nodes at `(1,y)` are sent to a `(0,y,0)` column.
Destination `(0,0,0)` has both `(0,0)` and `(1,0)` as sources.
```ignore
Switch{
	indexing: LinearTransform{
		source_size: [2, 8],
		target_size: [2],
		matrix: [
			[1, 0],
		],
	},
	patterns: [
		Composition{patterns:[
			LinearTransform{
				source_size: [2, 8],
				target_size: [8],
				matrix: [
					[0, 1],
				],
			},
			CartesianEmbedding{
				source_sides: [8,1,1],
				destination_sides: [8,8,8],
			},
		],middle_sizes:[8]},
		Composition{patterns:[
			LinearTransform{
				source_size: [2, 8],
				target_size: [8],
				matrix: [
					[0, 1],
				],
			},
			CartesianEmbedding{
				source_sides: [1,8,1],
				destination_sides: [8,8,8],
			},
		],middle_sizes:[8]},
	],
},
```

This example assigns 10 different RandomPermutations, and 2 uniforms depending on the `y` value, mentioned earlier.
```ignore
Switch{
	indexing: LinearTansform{
		source_size: [2, 12],
		target_size: [12],
		matrix: [
			[0, 1],
		],
	},
	patterns: [
		RandomPermutation,
	    Uniform
	],
	expand: [10,2], //put 10 RandomPermutations, followed by 2 Uniforms
}
```
 **/
#[derive(Debug,Quantifiable)]
pub struct Switch {
    indexing: Box<dyn Pattern>,
    patterns: Vec<Box<dyn Pattern>>,
    seed: Option<f64>,
}

impl GeneralPattern<usize, usize>for Switch {
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)
    {
        self.indexing.initialize(source_size,self.patterns.len(),topology,rng);

        let mut seed_generator = if let Some(seed) = self.seed{
            Some(StdRng::seed_from_u64(seed as u64))
        } else {
            None
        };
        for pattern in self.patterns.iter_mut() {
            if let Some( seed_generator) = seed_generator.as_mut(){
                let seed = seed_generator.next_u64();
                pattern.initialize(source_size,target_size,topology, &mut StdRng::seed_from_u64(seed));
            }else{
                pattern.initialize(source_size,target_size,topology, rng);
            }
        }
    }
    fn get_destination(&self, origin:usize, topology: Option<&dyn Topology>, rng: &mut StdRng)->usize
    {
        let index = self.indexing.get_destination(origin,topology,rng);
        self.patterns[index].get_destination(origin,topology,rng)
    }
}

impl Switch {
    pub(crate) fn new(arg:MetaPatternBuilderArgument) -> Switch
    {
        let mut indexing = None;
        let mut patterns= None;//:Option<Vec<Box<dyn Pattern>>> = None;
        let mut expand: Option<Vec<usize>> = None;
        let mut seed = None;

        match_object_panic!(arg.cv,"Switch",value,
			"indexing" => indexing = Some(new_pattern(MetaPatternBuilderArgument{cv:value,..arg})),
			"patterns" => patterns=Some( value.as_array().expect("bad value for patterns") ),
			"expand" => expand = Some(value.as_array().expect("bad value for expand").iter()
				.map(|v|v.as_usize().expect("bad value in expand")).collect()),
			"seed" => seed = Some(value.as_f64().expect("bad value for seed")),
		);
        let indexing = indexing.expect("Missing indexing in Switch.");
        let patterns = patterns.expect("Missing patterns in Switch.");
        let patterns = if let Some(expand) = expand {
            let mut new_patterns = vec![];
            for (index, pattern) in patterns.into_iter().enumerate() {
                for _ in 0..expand[index] {
                    new_patterns.push(new_pattern(MetaPatternBuilderArgument{cv:pattern,..arg}));
                }
            }
            new_patterns
        } else {
            patterns.iter().map(|pcv|new_pattern(MetaPatternBuilderArgument{cv:pcv,..arg})).collect()
        };
        Switch{
            indexing,
            patterns,
            seed,
        }
    }
}