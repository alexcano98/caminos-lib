use std::collections::VecDeque;
use std::path::Path;
use std::rc::Rc;
use crate::ConfigurationValue;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use crate::general_pattern::many_to_many_pattern::{ManyToManyParam, ManyToManyPattern};
use crate::general_pattern::prelude::Pattern;
use crate::{match_object_panic, AsMessage, Message, Time};
use crate::config::evaluate;
use crate::general_pattern::{new_many_to_many_pattern, new_pattern, GeneralPatternBuilderArgument};
use crate::measures::TrafficStatistics;
use crate::packet::ReferredPayload;
use crate::topology::Topology;
use crate::traffic::{new_traffic, TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};


///Scheduler trait
pub trait Scheduler: Traffic{
    ///Allocates traffic into the network
    fn allocate_next(&mut self, rng: &mut StdRng) -> bool;
}


/**
Allocates a list of traffics in the network following a FIFO order.
If there are not enough resources to allocate the traffic, it waits until the resources are available.
```ignore
FIFOScheduler{
    servers: 32,
	traffics: [Burst{...}, Burst{...}],
    resource_selection: RandomSelection{...},
    task_mapping: Identity,
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct FIFOScheduler
{
    ///Total servers in the network.
    total_servers: usize,
    ///List of applicable traffics.
    traffics: Vec<Box<dyn Traffic>>,
    ///For each traffic, the list of servers used to host the traffic.
    task_traffics: Vec<Vec<usize>>,
    ///Resource selection policy
    resource_selection: Box<dyn ManyToManyPattern>,
    ///Task mapping pattern
    task_mapping: Box<dyn Pattern>,
    ///List of servers in the network and the traffics they serve at each cycle
    server_occupation_map: Vec<Vec<usize>>,
    // ///Tasks per server.
    // tasks_per_server: usize,
    ///List containing the active traffics at the moment.
    active_traffics: Vec<usize>,
    ///Index of the next traffic to allocate
    next_traffic: usize,
    ///For each task, the index of the traffic that is generating messages.
    index_to_generate: Vec<VecDeque<usize>>,
}


impl Traffic for FIFOScheduler{
    fn generate_message(&mut self, origin: usize, cycle: Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Result<Rc<Message>, TrafficError> {
        let index_traffic = self.index_to_generate[origin].pop_front().unwrap();
        let sub_origin = self.task_traffics[index_traffic].iter().position(|&v| v == origin).unwrap();
        let message = self.traffics[index_traffic].generate_message(sub_origin, cycle, topology, rng)?;
        let destination_real =  self.task_traffics[index_traffic][message.destination];

        let mut payload = Vec::with_capacity(message.payload().len() + 4);
        let index_convert = index_traffic as u32;
        let i_bytes = bytemuck::bytes_of(&index_convert);
        payload.extend_from_slice(&i_bytes);
        payload.extend_from_slice(message.payload());

        Ok(Rc::new(Message{
            origin,
            destination: destination_real,
            payload,
            ..*message
        }))
    }

    fn probability_per_cycle(&self, task: usize) -> f32 {
        let task_traffics = &self.server_occupation_map[task];
        if task_traffics.is_empty(){
            return 0.0;
        }else {
            task_traffics.iter().map(|t| self.traffics[*t].probability_per_cycle(task)).sum::<f32>().min(1f32)
        }
    }

    fn consume(&mut self, task: usize, message: &dyn AsMessage, cycle: Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> bool {

        let index=  *bytemuck::try_from_bytes::<u32>(&message.payload()[0..4]).expect("Bad index in message for TrafficSum.") as usize;
        let sub_payload = &message.payload()[4..];

        if task != message.destination(){
            panic!("Task {} is not the destination of the message", task);
        }

        let mut sub_message = ReferredPayload::from(message);
        sub_message.payload = sub_payload;

        sub_message.origin = self.task_traffics[index].iter().position(|&v| v == sub_message.origin).unwrap();
        sub_message.destination = self.task_traffics[index].iter().position(|&v| v == task).unwrap();

        self.traffics[index].consume(sub_message.destination, &sub_message, cycle, topology, rng)
    }

    fn is_finished(&mut self, rng: Option<&mut StdRng>) -> bool {
        let rng = rng.unwrap();
        let finished = self.active_traffics.clone().into_iter().filter(|&t| self.traffics[t].is_finished(Some(rng))).collect::<Vec<usize>>();
        if finished.len() > 0 {
            self.active_traffics.retain(|t| !finished.contains(t));
            while self.allocate_next(rng){}
        }
        self.active_traffics.is_empty()
    }

    fn should_generate(&mut self, task: usize, _cycle: Time, _rng: &mut StdRng) -> bool {

        if self.index_to_generate[task].len() > 0{
            return true;
        }

        let traffics= self.server_occupation_map[task].clone();

        traffics.into_iter().for_each(|t| {
            let sub_task = self.task_traffics[t].iter().position(|&v| v == task);
            if  let Some(sub_task) = sub_task {
                if self.traffics[t].should_generate(sub_task, _cycle, _rng){
                    self.index_to_generate[task].push_back(t);
                }
            }
        });

        self.index_to_generate[task].len() > 0
    }

    fn task_state(&mut self, task: usize, cycle: Time) -> Option<TaskTrafficState> {
        let task_traffics = &self.server_occupation_map[task];
        if task_traffics.is_empty(){
            return Some(TaskTrafficState::UnspecifiedWait);
        }else {
            let mut state = TaskTrafficState::UnspecifiedWait;
            for &t in task_traffics{
                let sub_task = self.task_traffics[t].iter().position(|&v| v == task).unwrap();
                let traffic_state = self.traffics[t].task_state(sub_task, cycle);
                if let Some(traffic_state) = traffic_state{
                    if let TaskTrafficState::Generating = traffic_state{
                        state = traffic_state
                    }
                }
            }
            Some(state)
        }
    }

    fn number_tasks(&self) -> usize {
        self.total_servers
    }

    fn get_statistics(&self) -> Option<TrafficStatistics> {
        None
    }

}

impl Scheduler for FIFOScheduler{

    fn allocate_next(&mut self, rng: &mut StdRng) -> bool{

        let traffic_index = self.next_traffic;
        if traffic_index >= self.traffics.len(){
            return false;
        }
        let traffic = &self.traffics[traffic_index];
        let traffic_tasks = traffic.number_tasks();
        //panic if the number of tasks is 0
        assert!(traffic_tasks > 0);

        let available_servers = self.server_occupation_map.iter().enumerate().filter(|(_, v)| v.len() < 1).map(|(i, _)| i).collect::<Vec<usize>>();
        let resource_selection_params = ManyToManyParam{
            origin: None,
            current: None,
            destination: None,
            list: available_servers,
            extra: Some(traffic_tasks),
        };
        let selected_servers = self.resource_selection.get_destination(resource_selection_params, None, rng);

        if selected_servers.len() == traffic_tasks{

            let mut mapping = vec![None; traffic_tasks];
            self.task_mapping.initialize(traffic_tasks, traffic_tasks, None, rng);

            for task in 0..selected_servers.len(){
                let task_index_assigned = self.task_mapping.get_destination(task, None, rng);
                if mapping[task].is_some(){
                    panic!("Server {} is already assigned to task {}", task_index_assigned, mapping[task_index_assigned].unwrap());
                }
                mapping[task] = Some(task_index_assigned);
            }

            self.task_traffics[traffic_index] = mapping.iter().map(|v| selected_servers[v.unwrap()] ).collect();
            for server in &self.task_traffics[traffic_index]{
                self.server_occupation_map[*server].push(traffic_index);
            }
            self.active_traffics.push(traffic_index);
            self.next_traffic += 1;

            true

        } else {

            false
        }
    }
}

impl FIFOScheduler{

    pub fn new(arg:TrafficBuilderArgument) -> FIFOScheduler
    {
        let mut total_servers = None;
        let mut traffics: Option<Vec<Box<dyn Traffic>>> =None;
        let mut resource_selection = None;
        let mut task_mapping = None;
        // let mut tasks_per_server = 1; //default value

        match_object_panic!(arg.cv,"FIFOScheduler",value,
            "extra_number" => (),
            "servers" => total_servers = Some(value.as_usize().expect("bad value for servers")),
			"traffics" => traffics = {
                if let ConfigurationValue::Array(a ) = value{

                    Some(a.iter().map(|v| new_traffic(TrafficBuilderArgument{cv: v, rng: arg.rng, ..arg})).collect())

                } else if let ConfigurationValue::Expression(expr) = value{

                    if let Ok(ConfigurationValue::Array(lista)) = evaluate(expr, arg.cv, Path::new(&""))
                    {
                        Some(lista.iter().map(|a| new_traffic(TrafficBuilderArgument{cv: a, rng: arg.rng, ..arg})).collect())
                    }else{
                        panic!("bad expression for traffics")
                    }

                } else{
                    panic!("bad value for traffics")
                }
            },
            "resource_selection" => resource_selection = Some(new_many_to_many_pattern(GeneralPatternBuilderArgument{cv:value, plugs:arg.plugs})),
            "task_mapping" => task_mapping = Some(new_pattern(GeneralPatternBuilderArgument{cv:value, plugs:arg.plugs})),
            // "tasks_per_server" => tasks_per_server = value.as_usize().expect("bad value for tasks_per_server"),
		);
        let total_servers = total_servers.expect("servers is required");
        let traffics = traffics.expect("traffics is required");
        let task_traffics = vec![vec![]; traffics.len()];
        let mut resource_selection = resource_selection.expect("resource_selection is required");
        resource_selection.initialize(total_servers, total_servers, None, arg.rng);
        let task_mapping = task_mapping.expect("task_mapping is required");
        let server_occupation_map = vec![vec![]; total_servers];
        let active_traffics = vec![];
        let next_traffic = 0;
        let index_to_generate = vec![VecDeque::new(); total_servers];

        let mut sched = FIFOScheduler{
            total_servers,
            traffics,
            task_traffics,
            resource_selection,
            task_mapping,
            server_occupation_map,
            // tasks_per_server,
            active_traffics,
            next_traffic,
            index_to_generate,
        };
        let rng = arg.rng;
        while sched.allocate_next(rng){}
        sched
    }
}
#[allow(dead_code)]
pub struct FIFOSchedulerBuilderCV{
    pub servers: usize,
    pub traffics: Vec<ConfigurationValue>,
    pub resource_selection: ConfigurationValue,
    pub task_mapping: ConfigurationValue,
    // pub tasks_per_server: usize,
}

#[allow(dead_code)]
pub fn create_fifo_scheduler_cv(args: FIFOSchedulerBuilderCV) -> ConfigurationValue{
    ConfigurationValue::Object("FIFOScheduler".to_string(), vec![
        ("servers".to_string(), ConfigurationValue::Number(args.servers as f64)),
        ("traffics".to_string(), ConfigurationValue::Array(args.traffics)),
        ("resource_selection".to_string(), args.resource_selection),
        ("task_mapping".to_string(), args.task_mapping),
        // ("tasks_per_server".to_string(), args.tasks_per_server),
    ])
}


#[cfg(test)]
mod tests {
    use rand::prelude::StdRng;
    use rand::SeedableRng;
    use crate::config_parser::ConfigurationValue;
    use crate::config_parser::Token::Expression;
    use crate::traffic::collectives::get_all2all;
    use crate::traffic::schedulers::FIFOScheduler;
    use crate::traffic::{Traffic, TrafficBuilderArgument};

    #[test]
    fn test_fifo_scheduler() {
        let mut rng = StdRng::seed_from_u64(0);

        let all2all_tasks= 8;
        let all2allcv = get_all2all(all2all_tasks, 128, 1);

        let servers = 32;
        let cv = super::create_fifo_scheduler_cv(super::FIFOSchedulerBuilderCV{
            servers,
            traffics: vec![all2allcv.clone(), all2allcv],
            resource_selection: ConfigurationValue::Object("RandomSelection".to_string(), vec![]),
            task_mapping: ConfigurationValue::Object("Identity".to_string(), vec![]),
        });

        let mut scheduler = FIFOScheduler::new(TrafficBuilderArgument{
            cv: &cv,
            plugs: &Default::default(),
            topology: None,
            rng: &mut rng,
        });

        let mut messages = vec![];
        for i in 0..(all2all_tasks-1){ //the same number of messages for each task
            let mut to_gen = 0;
            assert_eq!(scheduler.is_finished(Some(&mut rng)), false);
            for i in 0..servers{
                if scheduler.should_generate(i, 0, &mut rng){
                    to_gen += 1;
                   messages.push(scheduler.generate_message(i, 0, None, &mut rng).unwrap());
                }
            }
            println!("scheduler: {:?}", scheduler);
            assert_eq!(to_gen, all2all_tasks *2);
        }

        assert_eq!(scheduler.is_finished(Some(&mut rng)), false);

        for message in &messages{
            assert_eq!(scheduler.consume(message.destination, message.as_ref(), 0, None, &mut rng), true);
        }

        assert_eq!(scheduler.is_finished(Some(&mut rng)), true);
    }
}
