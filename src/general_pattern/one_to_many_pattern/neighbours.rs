use std::convert::TryInto;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use crate::match_object_panic;
use crate::general_pattern::{GeneralPattern, GeneralPatternBuilderArgument};
use crate::topology::prelude::CartesianData;
use crate::topology::Topology;
use crate::ConfigurationValue;

///Auxiliar function
fn extend_vectors(vectors: Vec<Vec<i32>>, value: i32) -> Vec<Vec<i32>>
{
    let mut new_vectors = vec![];
    for mut vector in vectors
    {
        vector.push(value);
        new_vectors.push(vector);
    }
    new_vectors
}

///Auxiliar function to get the neighbours inside a grid from a vector of vectors
fn get_non_modular_neighbours_from_vector(sides: &Vec<usize>, vector_neighbours: &Vec<Vec<i32>>) -> Vec<Vec<usize>>
{
    let mut all_neighbours = vec![];
    let cartesian_data = CartesianData::new(&sides.clone());
    let size = sides.clone().into_iter().reduce(|a, b| a * b).unwrap();

    for i in 0..size{
        let i_coord = cartesian_data.unpack(i);
        let mut neighbours = vec![];
        //iter over all the vectors
        'outer: for j in vector_neighbours{
            let mut neighbour = vec![];
            for (index, coord) in i_coord.iter().enumerate(){
                let new_coord_value = *coord as i32 + j[index];
                if new_coord_value >= 0 && new_coord_value < sides[index] as i32{
                    neighbour.push(new_coord_value.try_into().unwrap());
                } else {
                    continue 'outer;
                }
            }
            neighbours.push(cartesian_data.pack(&neighbour));
        }
        all_neighbours.push(neighbours);
    }
    all_neighbours

}

///Auxiliar function to get the neighbours inside a grid from a vector of vectors
fn get_modular_neighbours_from_vector(sides: &Vec<usize>, vector_neighbours: &Vec<Vec<i32>>) -> Vec<Vec<usize>>
{
    let mut all_neighbours = vec![];
    let cartesian_data = CartesianData::new(&sides.clone());
    let size = sides.clone().into_iter().reduce(|a, b| a * b).unwrap();

    for i in 0..size{
        let i_coord = cartesian_data.unpack(i);
        let mut neighbours = vec![];
        //iter over all the vectors
        for j in vector_neighbours{
            let neighbour = j.iter().zip(i_coord.iter()).zip(sides.iter()).map(|((a, b), sides)| (a.rem_euclid(*sides as i32) as usize + b) % sides).collect::<Vec<usize>>();
            neighbours.push(cartesian_data.pack(&neighbour));
        }
        all_neighbours.push(neighbours);
    }
    all_neighbours
}


/**
Neighbours general_pattern that selects the neighbours of a node in a space.
```ignore
    ManhattanNeighbours{ //Iter the neighbours inside a manhattan distance. No wrap-around
        sides: [3,3],
        distance: 1,
    }
**/

#[derive(Debug, Quantifiable)]
pub struct ManhattanNeighbours
{
    neighbours: Vec<Vec<usize>>
}

impl GeneralPattern<usize, Vec<usize>> for ManhattanNeighbours{
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        //panic if source size is different to neighbour.len()
        assert_eq!(source_size, self.neighbours.len());
        assert_eq!(target_size, self.neighbours.len());
    }
    fn get_destination(&self, param: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        self.neighbours[param].clone()
    }
}

impl ManhattanNeighbours {
    pub fn new(arg: GeneralPatternBuilderArgument) -> ManhattanNeighbours {
        let mut distance = None;
        let mut sides = None;
        match_object_panic!(arg.cv,"ManhattanNeighbours",value,
            "sides" => sides = Some(value.as_array().expect("bad value for sides").iter().map(|v|v.as_usize().expect("bad value in sides")).collect()),
            "distance" => distance = Some(value.as_usize().expect("bad value for distance")),
        );
        let distance = distance.expect("There were no distance in configuration of KingNeighbours.");
        let sides: Vec<usize> = sides.expect("There were no sides in configuration of KingNeighbours.");

        let vector_neighbours = Self::get_vectors_in_manhattan_distance(&(sides.clone()), distance);
        let neighbours = get_non_modular_neighbours_from_vector(&sides, &vector_neighbours);

        ManhattanNeighbours {
            neighbours,
        }
    }

    /// sides are the sides of the space, manhattan_distance is the distance to be considered
    fn get_vectors_in_manhattan_distance(sides: &Vec<usize>, manhattan_distance: usize) -> Vec<Vec<i32>>
    {
        let vectors = Self::get_vectors_in_manhattan_distance_aux(sides, manhattan_distance);
        let mut vectors:Vec<Vec<i32>> = vectors.into_iter().filter(|e| !e.iter().all(|&i| i == 0)).collect(); //remove all 0s
        vectors.sort();
        vectors
    }


    /// sides are the sides of the space, manhattan_distance is the distance to be considered
    fn get_vectors_in_manhattan_distance_aux(sides: &[usize], manhattan_distance: usize) -> Vec<Vec<i32>>
    {
        let total_dist = manhattan_distance as i32;
        if manhattan_distance == 0
        {
            vec![vec![0;sides.len()]]
        } else if sides.len() == 1{
            (-total_dist..=total_dist).map(|i| vec![i] ).collect()
        } else {
            let mut vectors = vec![];
            for dist in 0..=total_dist
            {
                let vec = Self::get_vectors_in_manhattan_distance_aux(&sides[1..], (total_dist - dist) as usize);
                let vec_1 = extend_vectors(vec.clone(), dist);
                vectors.extend(vec_1);
                if dist != 0{
                    let vec_2 = extend_vectors(vec.clone(), -dist);
                    vectors.extend(vec_2);
                }
            }
            vectors
        }
    }
}

/**
Neighbours pattern that selects the neighbours of a node in a space.
```ignore
    KingNeighbours{ //Iter the neighbours inside a chessboard distance. No wrap-around
        sides: [3,3],
        distance: 1,
    }
**/
#[derive(Debug, Quantifiable)]
pub struct KingNeighbours
{
    neighbours: Vec<Vec<usize>>
}

impl GeneralPattern<usize, Vec<usize>> for KingNeighbours{
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        //panic if source size is different to neighbour.len()
        assert_eq!(source_size, self.neighbours.len());
        assert_eq!(target_size, self.neighbours.len());
    }
    fn get_destination(&self, param: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        self.neighbours[param].clone()
    }
}

impl KingNeighbours{
    pub fn new(arg: GeneralPatternBuilderArgument) -> KingNeighbours {
        let mut sides = None;
        let mut distance = None;
        match_object_panic!(arg.cv,"KingNeighbours",value,
            "sides" => sides = Some(value.as_array().expect("bad value for sides").iter().map(|v|v.as_usize().expect("bad value in sides")).collect()),
            "distance" => distance = Some(value.as_usize().expect("bad value for distance")),
        );
        let sides: Vec<usize> = sides.expect("There were no sides in configuration of KingNeighbours.");
        let distance = distance.expect("There were no distance in configuration of KingNeighbours.");

        let vector_neighbours = Self::get_vectors_in_king_distance(&(sides.clone()), distance);
        let neighbours = get_non_modular_neighbours_from_vector(&sides, &vector_neighbours);

        KingNeighbours {
            neighbours,
        }
    }

    /// sides are the sides of the space, chessboard_distance is the distance to be considered
    fn get_vectors_in_king_distance(sides: &[usize], chessboard_distance: usize) -> Vec<Vec<i32>>
    {
        let vectors = Self::get_vectors_in_king_distance_aux(sides, chessboard_distance);
        let mut vectors:Vec<Vec<i32>> = vectors.into_iter().filter(|e| !e.iter().all(|&i| i == 0)).collect(); //remove all 0s
        vectors.sort();
        vectors
    }

    fn get_vectors_in_king_distance_aux(sides: &[usize], chessboard_distance: usize) -> Vec<Vec<i32>>
    {
        let total_dist = chessboard_distance as i32;
        if chessboard_distance == 0
        {
            vec![vec![0;sides.len()]]
        }else if sides.len() == 1
        {
            (-total_dist..=total_dist).map(|i| vec![i] ).collect()
        } else {
            let mut vectors = vec![];
            let vec = Self::get_vectors_in_king_distance_aux(&sides[1..], chessboard_distance);

            let vec_1 = extend_vectors(vec.clone(), 0);
            vectors.extend(vec_1);

            for dist in 1..=total_dist
            {
                let vec_1 = extend_vectors(vec.clone(), dist);
                vectors.extend(vec_1);

                let vec_2 = extend_vectors(vec.clone(), -dist);
                vectors.extend(vec_2);
            }
            vectors
        }
    }

}

/**
Returns the neighbours in a hypercube
```ignore
    HypercubeNeighbours{}
```
 **/
#[derive(Debug, Quantifiable)]
pub struct HypercubeNeighbours
{
    neighbours: Vec<Vec<usize>>
}

impl GeneralPattern<usize, Vec<usize>> for HypercubeNeighbours{
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        //panic if source size is not a power of 2
        assert!(source_size.is_power_of_two());
        //panic if source size is different to neighbour.len()
        assert_eq!(source_size, target_size);
        self.neighbours = Self::get_neighbours(source_size);

    }
    fn get_destination(&self, param: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        self.neighbours[param].clone()
    }
}

impl HypercubeNeighbours{
    pub fn new(_arg: GeneralPatternBuilderArgument) -> HypercubeNeighbours {
        HypercubeNeighbours {
            neighbours: vec![]
        }
    }
    fn get_neighbours(source_size: usize) -> Vec<Vec<usize>> {
        let mut neighbours = vec![vec![]; source_size];
        let dimensions = source_size.ilog2();
        //calculate the neighbours
        for i in 0..source_size{
            let mut local_neighbours = vec![];
            for j in 0..dimensions{
                local_neighbours.push(i ^ (1 << j));
            }
            neighbours[i] = local_neighbours;
        }
        neighbours
    }
}

/**
Returns the neighbours in a binomial tree
```ignore
    BinomialTreeNeighbours{
        go_up: true, //aim the parent
    }
```
**/
#[derive(Debug, Quantifiable)]
pub struct BinomialTreeNeighbours
{
    neighbours: Vec<Vec<usize>>,
    go_up: bool,
}

impl GeneralPattern<usize, Vec<usize>> for BinomialTreeNeighbours{
    fn initialize(&mut self, source_size: usize, _target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        //panic if source size is not a power of 2
        assert!(source_size.is_power_of_two());
        self.neighbours = vec![vec![]; source_size];
        let dimensions = source_size.ilog2();
        //calculate the neighbours
        for i in 0..source_size{
            //get the index of the first 1 in the binary representation of i
            let mut index = -1;
            for j in 0..dimensions{
                if i & (1 << j) != 0{
                    index = j as i32;
                    break;
                }
            }
            for i in ((index+1) as u32)..dimensions{
                let neighbour = (i ^ (1 << index)) as usize;
                if self.go_up{
                    self.neighbours[i as usize].push(neighbour);
                } else {
                    self.neighbours[neighbour].push(i as usize);
                }
            }
        }
        println!("{:?}", self.neighbours);
    }
    fn get_destination(&self, param: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        self.neighbours[param].clone()
    }
}

impl BinomialTreeNeighbours{
    pub fn new(arg: GeneralPatternBuilderArgument) -> BinomialTreeNeighbours {
        let mut go_up = None;
        match_object_panic!(arg.cv,"BinomialTreeNeighbours",value,
            "go_up" => go_up = Some(value.as_bool().expect("bad value for go_up")),
        );
        let go_up = go_up.expect("There were no go_up in configuration of BinomialTreeNeighbours.");
        BinomialTreeNeighbours {
            neighbours: vec![],
            go_up
        }
    }
}

/**
Returns the neighbours in a binary tree
```ignore
    BinaryTreeNeighbours{
        go_up: true, //aim the parent
    }
```
**/
#[derive(Debug, Quantifiable)]
pub struct BinaryTreeNeighbours
{
    neighbours: Vec<Vec<usize>>,
    go_up: bool,
}

impl GeneralPattern<usize, Vec<usize>> for BinaryTreeNeighbours {
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        //panic if source size is not a power of 2
        // assert!(source_size.is_power_of_two());
        assert_eq!(source_size, target_size);
        self.neighbours = vec![vec![]; source_size];
        // let dimensions = source_size.ilog2();
        //calculate the neighbours
        for i in 0..source_size {
            let son = i << 1;
            if son < source_size {
                if self.go_up {
                    self.neighbours[i].push(son);
                } else {
                    self.neighbours[son].push(i);
                }
            }

            if son ^ 1 < source_size {
                if self.go_up {
                    self.neighbours[i].push(son ^ 1);
                } else {
                    self.neighbours[son ^ 1].push(i);
                }
            }
        }
    }
    fn get_destination(&self, param: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        self.neighbours[param].clone()
    }
}

impl BinaryTreeNeighbours{
    pub fn new(arg: GeneralPatternBuilderArgument) -> BinaryTreeNeighbours {
        let mut go_up = None;
        match_object_panic!(arg.cv,"BinaryTreeNeighbours",value,
            "go_up" => go_up = Some(value.as_bool().expect("bad value for go_up")),
        );
        let go_up = go_up.expect("There were no go_up in configuration of BinaryTreeNeighbours.");
        BinaryTreeNeighbours {
            neighbours: vec![],
            go_up
        }
    }
}

/**
Returns all the elements as neighbours
```ignore
    AllNeighbours{}
```
**/
#[derive(Debug, Quantifiable)]
pub struct AllNeighbours
{
    neighbours: Vec<Vec<usize>>,
}

impl GeneralPattern<usize, Vec<usize>> for AllNeighbours {
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        assert_eq!(source_size,target_size);

        self.neighbours = vec![vec![]; source_size];
        for i in 0..source_size {
            for j in 1..source_size {
                self.neighbours[i].push((i + j) % source_size);
            }
        }

    }
    fn get_destination(&self, param: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        self.neighbours[param].clone()
    }
}

impl AllNeighbours{
    pub fn new(_arg: GeneralPatternBuilderArgument) -> AllNeighbours {
        AllNeighbours {
            neighbours: vec![],
        }
    }
}

/**
Returns the neighbours in a grid for the given vector neighbours
```ignore
    GridNeighbours{
        sides: [3,3], //sides of the grid
        vector_neighbours: [[0,1],[1,0]], //vector neighbours
        modular: false, //if there are wrap-around neighbours
    }
```
**/
#[derive(Debug, Quantifiable)]
pub struct ImmediateNeighbours
{
    sides: Vec<usize>,
    vector_neighbours: Vec<Vec<i32>>,
    neighbours: Vec<Vec<usize>>,
    modular: bool,
}

impl GeneralPattern<usize, Vec<usize>> for ImmediateNeighbours {
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) {
        let all_size = self.sides.clone().into_iter().reduce(|a, b| (a * b)).unwrap();
        assert_eq!(source_size,target_size);
        assert_eq!(source_size, all_size);
        //the length of the vector_neighbours must be the same as the number of dimensions
        for i in self.vector_neighbours.iter(){
            assert_eq!(i.len(), self.sides.len());
        }

        if self.modular{
            self.neighbours = get_modular_neighbours_from_vector(&self.sides, &self.vector_neighbours);
        } else {
            self.neighbours = get_non_modular_neighbours_from_vector(&self.sides, &self.vector_neighbours);
        }

    }
    fn get_destination(&self, param: usize, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Vec<usize> {
        self.neighbours[param].clone()
    }
}

impl ImmediateNeighbours {
    pub fn new(arg: GeneralPatternBuilderArgument) -> ImmediateNeighbours {
        let mut sides = None;
        let mut vector_neighbours = None;
        let mut modular = None;
        match_object_panic!(arg.cv,"ImmediateNeighbours",value,
            "sides" => sides = Some(value.as_array().expect("bad value for sides").iter().map(|v|v.as_usize().expect("bad value in sides")).collect()),
            "vector_neighbours" => vector_neighbours = Some(value.as_array().expect("bad value for vector_neighbours").iter().map(|v|v.as_array().expect("bad value in vector_neighbours").iter().map(|v|v.as_i32().expect("bad value in vector_neighbours")).collect()).collect()),
            "modular" => modular = Some(value.as_bool().expect("bad value for modular")),
        );
        let sides: Vec<usize> = sides.expect("There were no sides in configuration of ImmediateNeighbours.");
        let vector_neighbours: Vec<Vec<i32>> = vector_neighbours.expect("There were no vector_neighbours in configuration of ImmediateNeighbours.");
        let modular = modular.expect("There were no modular in configuration of ImmediateNeighbours.");
        ImmediateNeighbours {
            sides,
            vector_neighbours,
            neighbours: vec![],
            modular
        }
    }
}

//ImmediateNeighbours CV builder
pub struct ImmediateNeighboursCVBuilder {
    pub(crate) sides: Vec<usize>,
    pub(crate) vector_neighbours: Vec<Vec<i32>>,
    pub(crate) modular: bool,
}

pub fn immediate_neighbours_cv_builder(arg: ImmediateNeighboursCVBuilder) -> ConfigurationValue {
    let sides = arg.sides.iter().map(|v| ConfigurationValue::Number(*v as f64)).collect();
    let vector_neighbours = arg.vector_neighbours.iter().map(|v| ConfigurationValue::Array(v.iter().map(|v| ConfigurationValue::Number(*v as f64)).collect())).collect();
    let boolean = if arg.modular { ConfigurationValue::True} else { ConfigurationValue::False };
    ConfigurationValue::Object("ImmediateNeighbours".to_string(),
       vec![
        ("sides".to_string(), ConfigurationValue::Array(sides)),
        ("vector_neighbours".to_string(), ConfigurationValue::Array(vector_neighbours)),
        ("modular".to_string(), boolean),
        ]
    )
}


#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use crate::general_pattern::GeneralPattern;

    #[test]
    fn test_manhattan_neighbours_distance_1() {

        let sides = vec![3];
        let manhattan_distance = 1;
        let vectors = super::ManhattanNeighbours::get_vectors_in_manhattan_distance(&sides, manhattan_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 2);
        let expected = vec![
            vec![-1],
            vec![1],
        ];
        for (index, vector) in vectors.iter().enumerate() {
            assert_eq!(vector, &expected[index]);
        }


        let sides = vec![3,3];
        let vectors = super::ManhattanNeighbours::get_vectors_in_manhattan_distance(&sides, manhattan_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 4);
        let expected = vec![
            vec![-1,0],
            vec![0,-1],
            vec![0,1],
            vec![1,0],
        ];
        for (index, vector) in vectors.iter().enumerate() {
            assert_eq!(vector, &expected[index]);
        }


        let sides = vec![3,3,3];
        let vectors = super::ManhattanNeighbours::get_vectors_in_manhattan_distance(&sides, manhattan_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 6);
        let expected = vec![
            vec![-1, 0, 0],
            vec![ 0,-1, 0],
            vec![ 0, 0,-1],
            vec![ 0, 0, 1],
            vec![ 0, 1, 0],
            vec![ 1, 0, 0],
        ];
        for (index, vector) in vectors.iter().enumerate() {
            assert_eq!(vector, &expected[index]);
        }
    }

    #[test]
    fn test_manhattan_neighbours_distance_3() {

        let sides = vec![8];
        let manhattan_distance = 3;
        let vectors = super::ManhattanNeighbours::get_vectors_in_manhattan_distance(&sides, manhattan_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 6);
        let expected = vec![
            vec![-3],
            vec![-2],
            vec![-1],
            vec![1],
            vec![2],
            vec![3],
        ];
        for (index, vector) in vectors.iter().enumerate() {
            assert_eq!(vector, &expected[index]);
        }


        let sides = vec![8,8];
        let vectors = super::ManhattanNeighbours::get_vectors_in_manhattan_distance(&sides, manhattan_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 24);
        let expected = vec![
            vec![-3,0],
            vec![-2,-1],
            vec![-2,0],
            vec![-2,1],

            vec![-1,-2],
            vec![-1,-1],
            vec![-1,0],
            vec![-1,1],
            vec![-1,2],


            vec![0,-3],
            vec![0,-2],
            vec![0,-1],
            vec![0,1],
            vec![0,2],
            vec![0,3],

            vec![1,-2],
            vec![1,-1],
            vec![1,0],
            vec![1,1],
            vec![1,2],

            vec![2,-1],
            vec![2,0],
            vec![2,1],

            vec![3,0],
        ];
        for (index, vector) in vectors.iter().enumerate() {
            assert_eq!(vector, &expected[index]);
        }
    }

    #[test]
    fn test_king_neighbours_distance_1() {

        let sides = vec![3];
        let chessboard_distance = 1;
        let vectors = super::KingNeighbours::get_vectors_in_king_distance(&sides, chessboard_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 2);
        let expected = vec![
            vec![-1],
            vec![1],
        ];
        for (index, vector) in vectors.iter().enumerate() {
            assert_eq!(vector, &expected[index]);
        }

        let sides = vec![3,3];
        let vectors = super::KingNeighbours::get_vectors_in_king_distance(&sides, chessboard_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 8);
        let expected = vec![
            vec![-1,-1],
            vec![-1,0],
            vec![-1,1],

            vec![0,-1],
            vec![0,1],

            vec![1,-1],
            vec![1,0],
            vec![1,1],
        ];
        for (index, vector) in vectors.iter().enumerate() {
            assert_eq!(vector, &expected[index]);
        }

        let sides = vec![3,3,3];
        let vectors = super::KingNeighbours::get_vectors_in_king_distance(&sides, chessboard_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 26);
        let expected = vec![
            vec![-1,-1,-1],
            vec![-1,-1, 0],
            vec![-1,-1, 1],
            vec![-1, 0,-1],
            vec![-1, 0, 0],
            vec![-1, 0, 1],
            vec![-1, 1,-1],
            vec![-1, 1, 0],
            vec![-1, 1, 1],


            vec![0,-1,-1],
            vec![0,-1, 0],
            vec![0,-1, 1],

            vec![0, 0,-1],
            vec![0, 0, 1],

            vec![0,1,-1],
            vec![0,1,0],
            vec![0,1,1],

            vec![1,-1,-1],
            vec![1,-1, 0],
            vec![1,-1, 1],
            vec![1, 0,-1],
            vec![1, 0, 0],
            vec![1, 0, 1],
            vec![1, 1,-1],
            vec![1, 1, 0],
            vec![1, 1, 1],

        ];
        for (index, vector) in vectors.iter().enumerate() {
            assert_eq!(vector, &expected[index]);
        }
    }

    #[test]
    fn test_king_neighbours_distance_2() {
        let sides = vec![8];
        let chessboard_distance = 2;
        let vectors = super::KingNeighbours::get_vectors_in_king_distance(&sides, chessboard_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 4);
        let expected = vec![
            vec![-2],
            vec![-1],
            vec![1],
            vec![2],
        ];
        for (index, vector) in vectors.iter().enumerate() {
            assert_eq!(vector, &expected[index]);
        }

        let sides = vec![8,8];
        let vectors = super::KingNeighbours::get_vectors_in_king_distance(&sides, chessboard_distance);
        println!("{:?}", vectors);
        assert_eq!(vectors.len(), 24);
    }

    #[test]
    fn test_hypercube_neighbours(){
        let neighbours = super::HypercubeNeighbours::get_neighbours(8);
        let nei = vec![
            vec![1, 2, 4],
            vec![0, 3, 5],
            vec![0, 3, 6],
            vec![1, 2, 7],
            vec![0, 5, 6],
            vec![1, 4, 7],
            vec![2, 4, 7],
            vec![3, 5, 6],
        ];

        for (index, vector) in neighbours.iter().enumerate() {
            //check if the vectors are the same, no matter the order
            assert_eq!(vector.len(), nei[index].len());
            for (i, value) in vector.iter().enumerate(){
                assert!(nei[index].contains(value));
            }
        }
    }

    //test the get_modular_neighbours_from_vector and get_non_modular_neighbours_from_vector
    #[test]
    fn test_get_neighbours_from_vector(){
        let sides = vec![3,3];
        let vector_neighbours = vec![
            vec![0,1],
            vec![1,0],
        ];
        let neighbours = super::get_non_modular_neighbours_from_vector(&sides, &vector_neighbours);
        let expected = vec![
            vec![1, 3], //0
            vec![2, 4], //1
            vec![5   ], //2
            vec![4, 6], //3
            vec![5, 7], //4
            vec![8   ], //5
            vec![7   ], //6
            vec![8   ], //7
            vec![], //8
        ];
        for (index, vector) in neighbours.iter().enumerate() {
            //check if the vectors are the same, no matter the order
            assert_eq!(vector.len(), expected[index].len());
            for (i, value) in vector.iter().enumerate(){
                assert!(expected[index].contains(value));
            }
        }

        let neighbours = super::get_modular_neighbours_from_vector(&sides, &vector_neighbours);
        let expected = vec![
            vec![1, 3], //0
            vec![2, 4], //1
            vec![0, 5], //2
            vec![4, 6], //3
            vec![5, 7], //4
            vec![3, 8], //5
            vec![7, 0], //6
            vec![8, 1], //7
            vec![6, 2], //8
        ];
        for (index, vector) in neighbours.iter().enumerate() {
            //check if the vectors are the same, no matter the order
            assert_eq!(vector.len(), expected[index].len());
            for (i, value) in vector.iter().enumerate(){
                assert!(expected[index].contains(value));
            }
        }
    }

    #[test]
    fn test_binomial_tree_neighbours(){
        let mut binomial_tree = super::BinomialTreeNeighbours{
            neighbours: vec![],
            go_up: true,
        };
        binomial_tree.initialize(8, 8, None, &mut rand::prelude::StdRng::seed_from_u64(0));
        let expected = vec![
            vec![1, 2, 4],
            vec![3, 5],
            vec![6],
            vec![7],
            vec![5],
            vec![6],
            vec![7],
            vec![],
        ];
        for i in 0..8{
            assert_eq!(binomial_tree.get_destination(i, None, &mut rand::prelude::StdRng::seed_from_u64(0)), expected[i]);
        }
    }
}