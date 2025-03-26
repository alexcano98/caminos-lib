use quantifiable_derive::Quantifiable;
use crate::match_object_panic;
//the derive macro
use super::prelude::*;
use crate::matrix::Matrix;
use crate::ConfigurationValue;



#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Tree{
    down_degree: usize,
    num_routers: usize,
    servers_per_router: usize,
    up_down_distances: Matrix<Option<(u8,u8)>>,
}

impl Topology for Tree{
    fn num_routers(&self) -> usize {
        self.num_routers
    }

    fn num_servers(&self) -> usize {
        self.num_routers * self.servers_per_router
    }

    fn neighbour(&self, router_index: usize, port: usize) -> (Location, usize) {
        if port >= self.ports(router_index){
            panic!("Port {} is not a valid port for router {}", port, router_index);
        }
        if port >= self.degree(router_index){
            (Location::ServerPort(router_index * self.servers_per_router + port - self.degree(router_index)), 1)
        }else {
            let level = Tree::full_h_k_tree_level(self.down_degree, router_index);
            if level == 0{
                (Location::RouterPort {
                    router_index: port +1,
                    router_port: 0,
                }, 0)
            }else if level == 1 && port == 0 {
                (Location::RouterPort {
                    router_index: 0,
                    router_port: router_index -1,
                }, 0)
            }else{
                let offset = router_index - Tree::full_h_k_tree_nodes(self.down_degree, level - 1);

                if port == 0{
                    (Location::RouterPort {
                        router_index: Tree::full_h_k_tree_nodes(self.down_degree, level - 2) + offset / self.down_degree,
                        router_port: offset % self.down_degree +1,
                    }, 0)
                }else {
                    //return child
                    (Location::RouterPort {
                        router_index: Tree::full_h_k_tree_nodes(self.down_degree, level) + offset * self.down_degree + port -1,
                        router_port: 0,
                    }, 0)

                }
            }
        }
    }

    fn server_neighbour(&self, server_index: usize) -> (Location, usize) {
        let router_index = server_index / self.servers_per_router;
        let server_port = server_index % self.servers_per_router;
        let location = Location::RouterPort {
            router_index,
            router_port: server_port + self.degree(router_index),
        };
        (location, 1)
    }

    fn diameter(&self) -> usize {
       self.compute_diameter()
    }

    fn distance(&self, origin: usize, destination: usize) -> usize {
        if origin == destination{
            0
        }else {
            self.up_down_distances.get(origin,destination).map(|(u,d)|u as usize + d as usize).unwrap()
        }
    }

    fn amount_shortest_paths(&self, _origin: usize, _destination: usize) -> usize {
        todo!()
    }

    fn average_amount_shortest_paths(&self) -> f32 {
        todo!()
    }

    fn maximum_degree(&self) -> usize {
        self.down_degree + 1
    }

    fn minimum_degree(&self) -> usize {
        1
    }

    fn degree(&self, router_index: usize) -> usize {
        //get routers at distance 1
        let mut degree = 0;
        for i in 0..self.num_routers{
            if i == router_index{
                continue;
            }
            //if updown distance sum is equal to 1, add 1 to the degree
            if self.up_down_distances.get(router_index,i).map(|(u,d)|u as usize + d as usize).unwrap() == 1{
                degree += 1;
            }
        }
        degree
    }

    fn ports(&self, router_index: usize) -> usize {
        self.degree(router_index) + self.servers_per_router
    }

    fn cartesian_data(&self) -> Option<&CartesianData> {
        todo!()
    }

    fn up_down_distance(&self, origin: usize, destination: usize) -> Option<(usize, usize)> {
        self.up_down_distances.get(origin,destination).map(|(u,d)|(u.into(),d.into()))
    }

    fn compute_diameter(&self) -> usize {
        //get the maximum distance from a router to another, from the up_down_distances matrix
        let mut diameter = 0;
        for i in 0..self.num_routers{
            for j in 0..self.num_routers{
                if i == j{
                    continue;
                }
                let distance = self.up_down_distances.get(i,j).map(|(u,d)|u as usize + d as usize).unwrap();
                if distance > diameter{
                    diameter = distance;
                }
            }
        }
        diameter
    }

    // fn compute_distance_matrix(&self, _class_weight: Option<&[usize]>) -> Matrix<usize> {
    //     //get the distance from the up_down_distances matrix
    //     let mut distance_matrix = Matrix::constant(0, self.num_routers, self.num_routers);
    //     for i in 0..self.num_routers{
    //         for j in 0..self.num_routers{
    //             if i == j{
    //                 continue;
    //             }
    //             let distance = self.up_down_distances.get(i,j).map(|(u,d)|u as usize + d as usize).unwrap();
    //             *distance_matrix.get_mut(i,j)  = distance;
    //         }
    //     }
    //     distance_matrix
    // }
}

impl Tree {
    pub fn new(arg:TopologyBuilderArgument) -> Tree{
        let mut down_degree = None;
        let mut num_routers = None;
        let mut servers_per_router = None;
        match_object_panic!(arg.cv,"Tree",value,
            "degree" => down_degree = Some(value.as_usize().expect("bad value for down_degree")),
            "num_routers" => num_routers = Some(value.as_usize().expect("bad value for num_routers")),
            "servers_per_router" => servers_per_router = Some(value.as_usize().expect("bad value for servers_per_router")),
        );
        let down_degree = down_degree.unwrap() -1; //the down_degree is the degree of the tree minus 1
        let num_routers = num_routers.unwrap();
        let servers_per_router = servers_per_router.unwrap();
        let up_down_distances = Tree::build_up_down_matrix(down_degree, num_routers);

        Tree{
            down_degree,
            num_routers,
            servers_per_router,
            up_down_distances,
        }
    }

    //Number of nodes in a tree at height h, where all switches (but leaves) have degree k+1
    fn full_h_k_tree_nodes(k: usize, h:usize) -> usize{
        // (k.pow(h as u32 + 1) - 1) / (k - 1)
        if h == 0{
            1
        }else {
            let mut height = 1;
            let mut total_switches = 1 + (k+1); //switches in level 0 + level 1
            let mut last_leaves = k +1; //switches in level 1
            while height < h{
                height += 1;
                last_leaves = k * last_leaves;
                total_switches += last_leaves;
            }
            total_switches
        }
    }

    //Level of a switch in a tree, where all switches (but leaves) have degree k+1
    fn full_h_k_tree_level(k: usize, switch:usize) -> usize{
        let mut last_switch_index = Tree::full_h_k_tree_nodes(k, 0) -1;
        let mut height = 0;
        while switch > last_switch_index {
            height += 1;
            last_switch_index = Tree::full_h_k_tree_nodes(k, height) -1;
        }
        height
    }

    fn build_up_down_matrix(down_degree: usize, num_routers: usize) -> Matrix<Option<(u8,u8)>>{
        let mut up_down_distances = Matrix::constant(None, num_routers, num_routers);
        let mut ancestors = vec![vec![];num_routers];
        // let mut h = Tree::full_h_k_tree_height(down_degree, num_routers - 1);

        for i in 0..num_routers{
            let mut parent = i;
            ancestors[i].push(parent); //push yourself
            let mut level = Tree::full_h_k_tree_level(down_degree, parent);
            let mut offset = if parent == 0{
                0
            } else {
                parent - Tree::full_h_k_tree_nodes(down_degree, level - 1)
            };

            while level > 1{
                level = level -1;
                offset = offset / down_degree;
                parent = Tree::full_h_k_tree_nodes(down_degree, level -1) + offset;
                ancestors[i].push(parent);
            }
            if i != 0 {
                ancestors[i].push(0); //push the root
            }
        }
        // println!("{:?}", ancestors);
        for i in 0..num_routers{
            for j in 0..num_routers{
                if i == j{
                    continue;
                }
                for (index, a) in ancestors[i].iter().enumerate(){
                    let is_ancestor = ancestors[j].iter().position(|x| x == a);
                    if is_ancestor.is_some(){
                        let up_distance = index as u8;
                        let down_distance = is_ancestor.unwrap() as u8;
                        up_down_distances.get_mut(i,j).replace((up_distance, down_distance));
                        break;
                    }
                }
            }
        }
        up_down_distances
    }
}


#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn test_up_down_matrix(){
        let matrix = Tree::build_up_down_matrix(2, 16);
        // println!("{:?}", matrix);
        assert_eq!(matrix.get(0,2).unwrap(), (0u8,1u8));
        assert_eq!(matrix.get(6,10).unwrap(), (2u8,3u8));
        assert_eq!(matrix.get(15,11).unwrap(), (3u8,3u8));

        let matrix = Tree::build_up_down_matrix(3, 18);
        // println!("{:?}", matrix);
        assert_eq!(matrix.get(0,2).unwrap(), (0u8,1u8));
        assert_eq!(matrix.get(6,10).unwrap(), (2u8,2u8));
        assert_eq!(matrix.get(12,11).unwrap(), (1u8,1u8));
        assert_eq!(matrix.get(13,12).unwrap(), (1u8,1u8));
        assert_eq!(matrix.get(17,16).unwrap(), (3u8,2u8));
    }
}