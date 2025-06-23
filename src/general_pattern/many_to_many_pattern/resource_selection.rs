use std::cmp;
use quantifiable_derive::Quantifiable;
use rand::prelude::{SliceRandom, StdRng};
use crate::general_pattern::{new_many_to_many_pattern, GeneralPattern, GeneralPatternBuilderArgument};
use crate::general_pattern::many_to_many_pattern::{ManyToManyParam, ManyToManyPattern};
use crate::match_object_panic;
use crate::config_parser::ConfigurationValue;
use crate::general_pattern::many_to_many_pattern::filters::IdentityFilter;
use crate::topology::prelude::CartesianData;
use crate::topology::Topology;


/**
Pattern which selects a number of elements from the list which are consecutive.
```ignore
    ConsecutiveSelection {}
```
**/
#[derive(Quantifiable, Debug)]
pub struct ConsecutiveSelection {}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for ConsecutiveSelection
{
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut rand::prelude::StdRng) {
        assert_eq!(source_size, target_size);
    }

    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, _rng: &mut rand::prelude::StdRng) -> Vec<usize> {
        //check that the list is ordered
        for i in 0..param.list.len()-1{
            assert!(param.list[i] < param.list[i+1]);
        }

        let mut selected = vec![];
        let mut index = 0;
        while index < param.list.len(){
            let element = param.list[index];
            let mut selected_element = vec![element];
            let mut element_coords = element;
            index += 1;
            while index < param.list.len(){
                let element_n = param.list[index];
                if element_n == element_coords + 1{
                    selected_element.push(element_n);
                    element_coords = element_n;
                    index += 1;
                    if selected_element.len() >= param.extra.unwrap(){
                        selected = selected_element.clone();
                        break;
                    }
                } else {
                    break;
                }
            }
            if &selected_element.len() >= &param.extra.unwrap() {
                selected = selected_element;
                break;
            }
        }
        selected
    }
}

impl ConsecutiveSelection {
    pub fn new(_arg: GeneralPatternBuilderArgument) -> ConsecutiveSelection {
        ConsecutiveSelection {}
    }
}


/**
Pattern which selects a number of elements from the list which are in the same row.
```ignore
    BlockSelection {
        block_size: 10,
    }
```
**/
#[derive(Quantifiable, Debug)]
pub struct BlockSelection
{
    pub(crate) block_size: usize,
    pub(crate) selection_inside_block: Box<dyn ManyToManyPattern>,
    pub(crate) number_of_blocks: usize,
    pub(crate) random_block_selection: bool,
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for BlockSelection
{
    fn initialize(&mut self, source_size: usize, target_size: usize, topology: Option<&dyn Topology>, rng: &mut StdRng) {
        //check if the source_size is the same as the target_size
        assert_eq!(source_size, target_size);
        self.number_of_blocks = source_size / self.block_size;
        self.selection_inside_block.initialize(source_size, target_size, topology, rng); //It is not self.block_size
    }

    fn get_destination(&self, param: ManyToManyParam, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Vec<usize> {
        //check that the list is ordered
        for i in 0..param.list.len()-1{
            assert!(param.list[i] < param.list[i+1]);
        }
        let mut block_occupation = vec![vec![]; self.number_of_blocks];
        for i in 0..param.list.len(){
            let element = param.list[i];
            let block = element / self.block_size;
            block_occupation[block].push(element);
        }
        let to_select = param.extra.unwrap();

        if param.list.len() < to_select{
            return vec![]; // Cant allocate it.
        }
        //select the block with most elements to allocate the elements
        let mut ordered_blocks = block_occupation.iter().enumerate().collect::<Vec<_>>();
        if self.random_block_selection{
            //discard full blocks
            // ordered_blocks.retain(|a| a.1.len() < self.block_size);
            ordered_blocks.shuffle(rng);
        }else {
            ordered_blocks.sort_by(|a, b| if a.1 != b.1 {b.1.len().cmp(&a.1.len())} else {a.0.cmp(&b.0)});
        }
        let mut partitions_ordered = ordered_blocks.iter().map(|a| a.1.clone()).collect::<Vec<Vec<usize>>>();
        let mut selected = vec![];
        let mut last =1; //just a random number

        while last != selected.len() && selected.len() < to_select{
            last = selected.len();
            let mut index_block = 0;
            while index_block < partitions_ordered.len() && selected.len() < to_select{
                let mut block_elements = partitions_ordered[index_block].clone();
                let param_filter_pattern = ManyToManyParam{ list: block_elements.clone(), extra: Some(cmp::min(to_select - selected.len(), block_elements.len())), ..Default::default()};
                let filtered = self.selection_inside_block.get_destination(param_filter_pattern, topology, rng);
                block_elements.retain( |a| !filtered.contains(a) );
                selected.extend(filtered);
                partitions_ordered[index_block] = block_elements;
                index_block += 1;
            }

            while selected.len() > to_select{
                selected.remove(selected.len() -1);
            }
        }

        if selected.len() < to_select{
            vec![]
        }else {
            selected.sort_by(|a, b| a.cmp(&b));
            selected
        }
    }
}

impl BlockSelection {
    pub fn new(arg: GeneralPatternBuilderArgument) -> BlockSelection {
        let mut block_size = None;
        let mut selection_inside_block: Option<Box<dyn ManyToManyPattern>> = Some(Box::new( IdentityFilter{}));
        let mut random_block_selection = false;
        match_object_panic!(arg.cv,"BlockSelection",value,
            "block_size" => block_size= Some(value.as_usize().unwrap()),
            "selection_inside_block" => selection_inside_block = Some(new_many_to_many_pattern(GeneralPatternBuilderArgument{cv: value, ..arg})),
            "random_block_selection" => random_block_selection = value.as_bool().unwrap(),
        );
        let block_size = block_size.expect("distance is required");
        let selection_inside_block = selection_inside_block.unwrap();
        BlockSelection { block_size, selection_inside_block, number_of_blocks: 0, random_block_selection }
    }
}

/**
Pattern which selects a number of elements from the list randomly.
```ignore
    RandomSelection {}
```
**/

#[derive(Quantifiable, Debug)]
pub struct RandomSelection {}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for RandomSelection
{
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut rand::prelude::StdRng) {
        assert_eq!(source_size, target_size);
    }

    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, rng: &mut rand::prelude::StdRng) -> Vec<usize> {
        let mut selected = vec![];
        let mut list = param.list.clone();
        //check that the size of the list is greater than the extra
        assert!(list.len() >= param.extra.unwrap());
        list.shuffle(rng);
        for i in 0..param.extra.unwrap(){
            selected.push(list[i]);
        }
        selected
    }
}

impl RandomSelection {
    pub fn new(_arg: GeneralPatternBuilderArgument) -> RandomSelection {
        RandomSelection {}
    }
}


/**
Pattern which partitions the network in Ltiles.
Specific for a 2D network.
```ignore
    LTile {}
```
**/

#[derive(Quantifiable, Debug)]
pub struct LTileSelection {
    servers_per_switch: usize,
    n: usize,
    origins: Vec<Vec<usize>>,
    vectors_from_origin: Vec<Vec<usize>>,
    cartesian_data: CartesianData,
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for LTileSelection
{
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        assert_eq!(source_size, target_size);
        //check that is a square number
        let n_switches = source_size/self.servers_per_switch;
        let n = (n_switches as f64).sqrt() as usize;
        assert_eq!(n_switches, (n as f64).powi(2) as usize);
        self.n = n;
        self.cartesian_data = CartesianData::new(&vec![n, n]);
        //put the (x,x) dots in the origins until  (n,n)
        let mut origins = vec![];
        let mut vectors_from_origin = vec![];
        for i in 0..n{
            origins.push(vec![i, i]);
        }
        self.origins = origins;
        for i in 0..(n/2){
            vectors_from_origin.push(vec![i, 0]);
        }

        for i in 1..(n/2 +1){
            vectors_from_origin.push(vec![0, i]);
        }
        self.vectors_from_origin = vectors_from_origin;
    }

    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        let list = param.list.clone();
        let mut points_to_origins = vec![vec![]; self.origins.len()];

        'outer: for server in list.into_iter(){
            let switch = server / self.servers_per_switch;
            let point = self.cartesian_data.unpack(switch);
            for j in 0..self.origins.len(){
                for v in 0..self.vectors_from_origin.len(){
                    let new_point = self.origins[j].iter().zip(self.vectors_from_origin[v].iter()).map(|(a, b)| (a + b) % self.n ).collect::<Vec<usize>>();
                    if new_point == point{
                        points_to_origins[j].push(server);
                        continue 'outer;
                    }
                }
            }
        }
        //Return the elements with the origin point with the most elements
        let mut point = 0;
        let mut elements = points_to_origins[0].len();
        for i in 1..points_to_origins.len(){
            if points_to_origins[i].len() > elements{
                point = i;
                elements = points_to_origins[i].len();
            }
        }
       //return the selected points sorted
        let mut ret = points_to_origins[point].clone();
        ret.sort_by(|a, b| a.cmp(b));
        //return the first extra elements
        ret.into_iter().take(param.extra.unwrap()).collect()
    }
}

impl LTileSelection {
    pub fn new(arg: GeneralPatternBuilderArgument) -> LTileSelection {
        let mut servers_per_switch = None;
        match_object_panic!(arg.cv,"LTileSelection",value,
            "servers_per_switch" => servers_per_switch= Some(value.as_usize().unwrap()),
        );
        let servers_per_switch = servers_per_switch.expect("servers_per_switch is required");
        LTileSelection {
            servers_per_switch,
            n: 0,
            origins: vec![],
            vectors_from_origin: vec![],
            cartesian_data: CartesianData::new(&vec![]),
        }
    }
}


/**
Pattern which partitions the network in Diagonals
```ignore
    DiagonalSelection {}
```
**/

#[derive(Quantifiable, Debug)]
pub struct DiagonalSelection {
    servers_per_switch: usize,
    n: usize,
    origins: Vec<Vec<usize>>,
    cartesian_data: CartesianData,
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for DiagonalSelection{
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        assert_eq!(source_size, target_size);
        //check that is a square number
        let n_switches = source_size/self.servers_per_switch;
        let n = (n_switches as f64).sqrt() as usize;
        assert_eq!(n_switches, (n as f64).powi(2) as usize);
        self.n = n;
        self.cartesian_data = CartesianData::new(&vec![n, n]);

        let mut origins = vec![];
        for i in 0..n{
            origins.push(vec![i, 0]);
        }
        self.origins = origins;
    }

    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        let list = param.list.clone();
        let mut points_to_origins = vec![vec![]; self.origins.len()];
        let diagonal_vector = vec![1; self.origins[0].len()];

        'outer: for server in list.into_iter(){
            let switch = server / self.servers_per_switch;
            let point = self.cartesian_data.unpack(switch);
            //apply up to n-1 times the diagonal vector to each origin to get the point where the server is
            for j in 0..self.origins.len(){
                let mut new_point = self.origins[j].clone();
                for _ in 0..self.n{
                    new_point = new_point.iter().zip(diagonal_vector.iter()).map(|(a, b)| (a + b) % self.n ).collect::<Vec<usize>>();
                    if new_point == point{
                        points_to_origins[j].push(server);
                        continue 'outer;
                    }
                }
            }

        }
        //Return the elements with the origin point with the most elements
        let mut point = 0;
        let mut elements = points_to_origins[0].len();
        for i in 1..points_to_origins.len(){
            if points_to_origins[i].len() > elements{
                point = i;
                elements = points_to_origins[i].len();
            }
        }
        //return the selected points sorted
        let mut ret = points_to_origins[point].clone();
        ret.sort_by(|a, b| a.cmp(b));
        //return the first extra elements
        ret.into_iter().take(param.extra.unwrap()).collect()
    }
}

impl DiagonalSelection {
    pub fn new(arg: GeneralPatternBuilderArgument) -> DiagonalSelection {
        let mut servers_per_switch = None;
        match_object_panic!(arg.cv,"DiagonalSelection",value,
            "servers_per_switch" => servers_per_switch= Some(value.as_usize().unwrap()),
        );
        let servers_per_switch = servers_per_switch.expect("servers_per_switch is required");
        DiagonalSelection {
            servers_per_switch,
            n: 0,
            origins: vec![],
            cartesian_data: CartesianData::new(&vec![]),
        }
    }
}

/**
    Pattern which is composed of two patterns.
    The first one selects a block of elements, and the second pattern selects a number of elements inside that block.
    It repeatedly calls the block_selection pattern until it has selected the required number of elements.
    ```ignore
        IterBlockSelection{
            block_size: 8,
            block_selection: DiagonalSelection{servers_per_switch: 8},
            selection_inside_block: MinFilter{}, // select the lowest at each diagonal.
        }
**/

#[derive(Quantifiable, Debug)]
pub struct IterBlockSelection {
    pub(crate) block_size: usize,
    pub(crate) block_selection: Box<dyn ManyToManyPattern>,
    pub(crate) selection_inside_block: Box<dyn ManyToManyPattern>,
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for IterBlockSelection {
    fn initialize(&mut self, source_size: usize, target_size: usize, topology: Option<&dyn Topology>, rng: &mut StdRng) {
        assert_eq!(source_size, target_size);
        self.block_selection.initialize(source_size, target_size, topology, rng);
        self.selection_inside_block.initialize(source_size, target_size, topology, rng);
    }

    fn get_destination(&self, param: ManyToManyParam, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Vec<usize> {
        let mut available_servers = param.list.clone();
        let to_select = param.extra.unwrap();
        let mut selected_servers = vec![];

        while to_select > selected_servers.len(){
            let block_param = ManyToManyParam {
                list: available_servers.clone(),
                extra: Some(self.block_size),
                ..param
            };
            let mut block = self.block_selection.get_destination(block_param, topology, rng);
            if block.is_empty() {
                return vec![];
            }
            let inside_param = ManyToManyParam {
                list: block.clone(),
                ..param
            };
            let servers_to_select = self.selection_inside_block.get_destination(inside_param, topology, rng);
            //remove from servers the selected servers
            available_servers.retain(|a| !servers_to_select.contains(a));
            selected_servers.extend(servers_to_select);
        }
        //sort the selected servers
        selected_servers.sort_by(|a, b| a.cmp(b));
        return selected_servers.into_iter().take(to_select).collect();
    }
}

impl IterBlockSelection {
    pub fn new(arg: GeneralPatternBuilderArgument) -> IterBlockSelection {
        let mut block_size = None;
        let mut block_selection: Option<Box<dyn ManyToManyPattern>> = Some(Box::new(IdentityFilter{}));
        let mut selection_inside_block: Option<Box<dyn ManyToManyPattern>> = Some(Box::new(IdentityFilter{}));
        match_object_panic!(arg.cv,"IterBlockSelection",value,
            "block_size" => block_size= Some(value.as_usize().unwrap()),
            "block_selection" => block_selection = Some(new_many_to_many_pattern(GeneralPatternBuilderArgument{cv: value, ..arg})),
            "selection_inside_block" => selection_inside_block = Some(new_many_to_many_pattern(GeneralPatternBuilderArgument{cv: value, ..arg})),
        );
        let block_size = block_size.expect("block_size is required");
        let block_selection = block_selection.unwrap();
        let selection_inside_block = selection_inside_block.unwrap();
        IterBlockSelection { block_size, block_selection, selection_inside_block }
    }
}



#[cfg(test)]
mod test {
    use rand::SeedableRng;
    use crate::general_pattern::GeneralPattern;
    use crate::general_pattern::many_to_many_pattern::filters::{IdentityFilter, MinFilter};
    use crate::topology::prelude::CartesianData;

    #[test]
    fn test_consecutive_selection(){
        use crate::general_pattern::many_to_many_pattern::resource_selection::ConsecutiveSelection;
        use crate::general_pattern::GeneralPattern;
        use crate::general_pattern::many_to_many_pattern::ManyToManyParam;

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mut consecutive_selection = ConsecutiveSelection{};
        consecutive_selection.initialize(10, 10, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            extra: Some(3),
        };
        let selected = consecutive_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![1, 2, 3]);
    }

    #[test]
    fn test_block_selection(){
        use crate::general_pattern::many_to_many_pattern::resource_selection::BlockSelection;
        use crate::general_pattern::GeneralPattern;
        use crate::general_pattern::many_to_many_pattern::ManyToManyParam;

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mut block_selection = BlockSelection {
            block_size: 2,
            selection_inside_block: Box::new(IdentityFilter{}),
            number_of_blocks: 0,
            random_block_selection: false,
        };
        block_selection.initialize(1000, 1000, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            extra: Some(3),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 1, 2]);

        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![9, 10, 11, 12, 13],
            extra: Some(3),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![10, 11, 12]);

        //Second part of the test
        let mut block_selection = BlockSelection {
            block_size: 2,
            selection_inside_block: Box::new(MinFilter{}),
            number_of_blocks: 0,
            random_block_selection: false,
        };
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            extra: Some(4),
        };
        block_selection.initialize(1000, 1000, None, &mut rng);
        let selected = block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 2, 4, 6]);
    }

    #[test]
    fn test_block_selection_2(){
        use crate::general_pattern::many_to_many_pattern::resource_selection::BlockSelection;
        use crate::general_pattern::GeneralPattern;
        use crate::general_pattern::many_to_many_pattern::ManyToManyParam;

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mut block_selection = BlockSelection {
            block_size: 64,
            selection_inside_block: Box::new(IdentityFilter{}),
            number_of_blocks: 0,
            random_block_selection: false,
        };
        block_selection.initialize(512, 512, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![0,1,2,65,66,67,68,69,70,71,72,99,100,121],
            extra: Some(2),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![65, 66]);
        assert_eq!(2, selected.len());


        let mut block_selection = BlockSelection {
            block_size: 64,
            selection_inside_block: Box::new(MinFilter{}),
            number_of_blocks: 0,
            random_block_selection: false,
        };
        block_selection.initialize(512, 512, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![0,1,2,65,66,67,68,69,70,71,72,99,100,121],
            extra: Some(2),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 65]);
        assert_eq!(2, selected.len());


        let mut block_selection = BlockSelection {
            block_size: 256,
            selection_inside_block: Box::new(BlockSelection {
                block_size: 64,
                selection_inside_block: Box::new(MinFilter{}),
                number_of_blocks: 0,
                random_block_selection: false,
            }),
            number_of_blocks: 0,
            random_block_selection: false,
        };

        block_selection.initialize(512, 512, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: (0..256).collect(),
            extra: Some(64),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207]);
        assert_eq!(64, selected.len());


        let mut block_selection = BlockSelection {
            block_size: 128,
            selection_inside_block: Box::new(BlockSelection {
                block_size: 64,
                selection_inside_block: Box::new(MinFilter{}),
                number_of_blocks: 0,
                random_block_selection: false,
            }),
            number_of_blocks: 0,
            random_block_selection: false,
        };

        block_selection.initialize(512, 512, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: (0..256).collect(),
            extra: Some(64),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95]);
        assert_eq!(64, selected.len());

        block_selection.initialize(512, 512, None, &mut rng);
        let list_assigned = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95];
        let left = (0..256).filter(|a| !list_assigned.contains(a)).collect::<Vec<usize>>();
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: left,
            extra: Some(64),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223]);
        assert_eq!(64, selected.len());


        let mut list_assigned = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95];
        list_assigned.extend_from_slice(&selected);

        let left = (0..256).filter(|a| !list_assigned.contains(a)).collect::<Vec::<usize>>();
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: left,
            extra: Some(64),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127]);
        assert_eq!(64, selected.len());

    }

    #[test]
    fn test_random_block_selection(){
        use crate::general_pattern::many_to_many_pattern::resource_selection::BlockSelection;
        use crate::general_pattern::GeneralPattern;
        use crate::general_pattern::many_to_many_pattern::ManyToManyParam;

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let mut block_selection = BlockSelection {
            block_size: 2,
            selection_inside_block: Box::new(IdentityFilter{}),
            number_of_blocks: 0,
            random_block_selection: true,
        };
        block_selection.initialize(1000, 1000, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,],
            extra: Some(2),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        println!("{:?}", selected);
        assert_eq!(selected.len(), 2);

        let mut block_selection = BlockSelection {
            block_size: 5,
            selection_inside_block: Box::new(IdentityFilter{}),
            number_of_blocks: 0,
            random_block_selection: true,
        };
        block_selection.initialize(1000, 1000, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
            extra: Some(5),
        };
        let selected = block_selection.get_destination(param, None, &mut rng);
        println!("{:?}", selected);
        assert_eq!(selected.len(), 5);
    }


    #[test]
    fn test_random_selection(){
        use crate::general_pattern::many_to_many_pattern::resource_selection::RandomSelection;
        use crate::general_pattern::GeneralPattern;
        use crate::general_pattern::many_to_many_pattern::ManyToManyParam;

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mut random_selection = RandomSelection{};
        random_selection.initialize(10, 10, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            extra: Some(3),
        };
        let selected = random_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn test_ltile_selection(){
        use crate::general_pattern::many_to_many_pattern::resource_selection::LTileSelection;
        use crate::general_pattern::GeneralPattern;
        use crate::general_pattern::many_to_many_pattern::ManyToManyParam;

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mut ltile_selection = LTileSelection{
            servers_per_switch: 8,
            n: 0,
            origins: vec![],
            vectors_from_origin: vec![],
            cartesian_data: CartesianData::new(&vec![]),
        };
        ltile_selection.initialize(512, 512, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            extra: Some(3),
        };
        let selected = ltile_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 1, 2]);

        let list = (0..512).collect::<Vec<usize>>();
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list,
            extra: Some(64),
        };
        let selected = ltile_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 64, 65, 66, 67, 68, 69, 70, 71, 128, 129, 130, 131, 132, 133, 134, 135, 192, 193, 194, 195, 196, 197, 198, 199, 256, 257, 258, 259, 260, 261, 262, 263]);

        let mut list = (0..512).collect::<Vec<usize>>();
        list.retain(|a| !selected.contains(a));
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list,
            extra: Some(64),
        };
        let selected = ltile_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 136, 137, 138, 139, 140, 141, 142, 143, 200, 201, 202, 203, 204, 205, 206, 207, 264, 265, 266, 267, 268, 269, 270, 271, 328, 329, 330, 331, 332, 333, 334, 335]);
    }

    #[test]
    fn test_diagonal_selection(){
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mut diagonal_selection = crate::general_pattern::many_to_many_pattern::resource_selection::DiagonalSelection{
            servers_per_switch: 8,
            n: 0,
            origins: vec![],
            cartesian_data: CartesianData::new(&vec![]),
        };
        diagonal_selection.initialize(512, 512, None, &mut rng);
        let list = (0..512).collect::<Vec<usize>>();
        let param = crate::general_pattern::many_to_many_pattern::ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list,
            extra: Some(3),
        };
        let selected = diagonal_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 1, 2]);

        let mut list = (0..512).collect::<Vec<usize>>();
        list.retain(|a| !selected.contains(a));
        let param = crate::general_pattern::many_to_many_pattern::ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: list.clone(),
            extra: Some(3),
        };
        let selected = diagonal_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![8, 9, 10]);

        let param = crate::general_pattern::many_to_many_pattern::ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list,
            extra: Some(64),
        };
        let selected = diagonal_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![8, 9, 10, 11, 12, 13, 14, 15, 80, 81, 82, 83, 84, 85, 86, 87, 152, 153, 154, 155, 156, 157, 158, 159, 224, 225, 226, 227, 228, 229, 230, 231, 296, 297, 298, 299, 300, 301, 302, 303, 368, 369, 370, 371, 372, 373, 374, 375, 440, 441, 442, 443, 444, 445, 446, 447, 448, 449, 450, 451, 452, 453, 454, 455]);
    }

    #[test]
    fn test_iter_block_selection(){
        use crate::general_pattern::many_to_many_pattern::resource_selection::IterBlockSelection;
        use crate::general_pattern::GeneralPattern;
        use crate::general_pattern::many_to_many_pattern::ManyToManyParam;

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mut iter_block_selection = IterBlockSelection {
            block_size: 4,
            block_selection: Box::new(crate::general_pattern::many_to_many_pattern::resource_selection::BlockSelection {
                block_size: 2,
                selection_inside_block: Box::new(crate::general_pattern::many_to_many_pattern::filters::IdentityFilter{}),
                number_of_blocks: 0,
                random_block_selection: false,
            }),
            selection_inside_block: Box::new(crate::general_pattern::many_to_many_pattern::filters::MinFilter{}),
        };
        iter_block_selection.initialize(1000, 1000, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            extra: Some(3),
        };
        let selected = iter_block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 2, 4]);
    }

    #[test]
    fn test_iter_block_selection_2(){
        use crate::general_pattern::many_to_many_pattern::resource_selection::IterBlockSelection;
        use crate::general_pattern::GeneralPattern;
        use crate::general_pattern::many_to_many_pattern::ManyToManyParam;

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mut iter_block_selection = IterBlockSelection {
            block_size: 4,
            block_selection: Box::new(crate::general_pattern::many_to_many_pattern::resource_selection::BlockSelection {
                block_size: 4,
                selection_inside_block: Box::new(IdentityFilter{}),
                number_of_blocks: 0, //it will be initialized later...
                random_block_selection: false,
            }),
            selection_inside_block: Box::new(MinFilter{}),
        };
        iter_block_selection.initialize(1000, 1000, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19],
            extra: Some(7),
        };
        let selected = iter_block_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![0, 1, 4, 5, 8, 12, 16]);
    }
}