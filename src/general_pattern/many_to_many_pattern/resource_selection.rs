use quantifiable_derive::Quantifiable;
use rand::prelude::SliceRandom;
use crate::general_pattern::{GeneralPattern, GeneralPatternBuilderArgument};
use crate::general_pattern::many_to_many_pattern::ManyToManyParam;
use crate::match_object_panic;
use crate::config_parser::ConfigurationValue;
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
    RowSelection {
        sides: [10, 10, 10],
    }
```
**/
#[derive(Quantifiable, Debug)]
pub struct RowSelection
{
    pub(crate) cartesian_data: CartesianData,
}

impl GeneralPattern<ManyToManyParam, Vec<usize>> for RowSelection
{
    fn initialize(&mut self, source_size: usize, target_size: usize, _topology: Option<&dyn Topology>, _rng: &mut rand::prelude::StdRng) {
        //check if the source_size is the same as the target_size
        assert_eq!(source_size, target_size);
        assert_eq!(self.cartesian_data.size, source_size);
    }

    fn get_destination(&self, param: ManyToManyParam, _topology: Option<&dyn Topology>, _rng: &mut rand::prelude::StdRng) -> Vec<usize> {
        //check that the list is ordered
        for i in 0..param.list.len()-1{
            assert!(param.list[i] < param.list[i+1]);
        }

        let to_select = param.extra.unwrap();
        let mut list_index = 0;

        while list_index < param.list.len(){
            let element = param.list[list_index];
            let mut selected = vec![element];
            let ele_coords = self.cartesian_data.unpack(element);
            let mut seleted = 1;
            while seleted < to_select{
                list_index +=1;
                let element_n = param.list[list_index];
                let ele_coords_n = self.cartesian_data.unpack(element_n);
                if ele_coords[1] == ele_coords_n[1]{
                    selected.push(element_n);
                    seleted += 1;
                } else {
                    break;
                }
            }
            if seleted == to_select{
                return selected;
            }
        }

        vec![]
    }
}

impl RowSelection {
    pub fn new(arg: GeneralPatternBuilderArgument) -> RowSelection {
        let mut sides = None;
        match_object_panic!(arg.cv,"RowSelection",value,
            "sides" => sides= Some(value.as_array().unwrap().iter().map(|v| v.as_usize().unwrap() ).collect::<Vec<usize>>()),
        );
        let sides = sides.expect("distance is required");
        let cartesian_data = CartesianData::new(&sides);
        RowSelection { cartesian_data }
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

#[cfg(test)]
mod test {
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
    fn test_row_selection(){
        use crate::general_pattern::many_to_many_pattern::resource_selection::RowSelection;
        use crate::general_pattern::GeneralPattern;
        use crate::general_pattern::many_to_many_pattern::ManyToManyParam;
        use crate::topology::prelude::CartesianData;

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let mut row_selection = RowSelection{cartesian_data: CartesianData::new(&[10, 10, 10])};
        row_selection.initialize(1000, 1000, None, &mut rng);
        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            extra: Some(3),
        };
        let selected = row_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![1, 2, 3]);

        let param = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: vec![8, 9, 10, 11, 12, 13],
            extra: Some(3),
        };
        let selected = row_selection.get_destination(param, None, &mut rng);
        assert_eq!(selected, vec![10, 11, 12]);
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
}