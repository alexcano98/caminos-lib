use crate::general_pattern::pattern::extra::get_candidates_selection;
use crate::AsMessage;
use std::rc::Rc;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use crate::config_parser::ConfigurationValue;
use crate::{match_object_panic, Message, Time};
use crate::general_pattern::one_to_many_pattern::neighbours::{immediate_neighbours_cv_builder, ImmediateNeighboursCVBuilder};
use crate::topology::Topology;
use crate::traffic::{new_traffic, TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};
use crate::traffic::basic::{build_send_message_to_vector_cv, SendMessageToVectorCVBuilder};
use crate::traffic::extra::{get_message_size_modifier, get_traffic_manager, BuildMessageSizeModifierCVArgs, BuildTrafficManagerCVArgs};
use crate::traffic::sequences::{BuilderMessageTaskSequenceCVArgs, get_traffic_message_task_sequence};
use crate::traffic::TaskTrafficState::{UnspecifiedWait, WaitingData};

/**
Introduces a barrier when all the tasks has sent a number of messages.
Tasks will generate messages again when all the messages are consumed.
```ignore
MessageBarrier{
	traffic: HomogeneousTraffic{...},
	tasks: 1000,
	messages_per_task_to_wait: 10,
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct MessageBarrier
{
    ///Number of tasks applying this traffic.
    tasks: usize,
    ///Traffic
    traffic: Box<dyn Traffic>,
    ///The number of messages to send per iteration
    messages_per_task_to_wait: usize,
    ///Total sent
    total_sent_per_task: Vec<usize>,
    ///Total sent
    total_sent: usize,
    ///Total consumed
    total_consumed: usize,
    ///Consumed messages in the barrier
    total_consumed_per_task: Vec<usize>,
    ///Messages to consume to go waiting
    expected_messages_to_consume_to_wait: Option<usize>,
}

impl Traffic for MessageBarrier
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        let message = self.traffic.generate_message(origin,cycle,topology,rng);
        // if !message.is_err(){
            self.total_sent += 1;
            self.total_sent_per_task[origin] += 1;
        // }
        if message.as_ref().is_err_and(|a| matches!(a, TrafficError::SelfMessage) )
        {
            self.total_consumed += 1;
            self.total_consumed_per_task[origin] += 1;
            if self.total_sent == self.total_consumed && self.messages_per_task_to_wait * self.tasks == self.total_sent {
                self.total_sent = 0;
                self.total_consumed = 0;
                self.total_sent_per_task = vec![0; self.tasks];
                self.total_consumed_per_task = vec![0; self.tasks];
            }
        }
        message
    }
    fn probability_per_cycle(&self, task:usize) -> f32 //should i check the task?
    {
        if self.total_sent_per_task[task] <= self.messages_per_task_to_wait {

            self.traffic.probability_per_cycle(task)

        } else {

            0.0
        }
    }

    fn should_generate(&mut self, task:usize, cycle:Time, rng: &mut StdRng) -> bool
    {
        self.total_sent_per_task[task] < self.messages_per_task_to_wait && self.traffic.should_generate(task, cycle, rng)
    }

    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> bool
    {
        self.total_consumed += 1;
        self.total_consumed_per_task[task] += 1;
        if self.total_sent == self.total_consumed && self.messages_per_task_to_wait * self.tasks == self.total_sent {
            self.total_sent = 0;
            self.total_consumed = 0;
            self.total_sent_per_task = vec![0; self.tasks];
            self.total_consumed_per_task = vec![0; self.tasks];
        }
        self.traffic.consume(task, message, cycle, topology, rng)
    }
    fn is_finished(&mut self, rng: Option<&mut StdRng>) -> bool
    {
        self.traffic.is_finished(rng)
    }
    fn task_state(&mut self, task:usize, cycle:Time) -> Option<TaskTrafficState>
    {
        if self.total_sent_per_task[task] < self.messages_per_task_to_wait {
            self.traffic.task_state(task, cycle)
        } else {
            if let Some(expected_messages_to_consume) = self.expected_messages_to_consume_to_wait {
                return if self.total_consumed_per_task[task] < expected_messages_to_consume {
                    Some(WaitingData)
                } else {
                    Some(UnspecifiedWait)
                }
            }
            Some(UnspecifiedWait)
        }
    }

    fn number_tasks(&self) -> usize {
        self.tasks
    }
}

impl MessageBarrier
{
    pub fn new(mut arg:TrafficBuilderArgument) -> MessageBarrier
    {
        let mut tasks=None;
        let mut traffic = None;
        let mut messages_per_task_to_wait = None;
        let mut expected_messages_to_consume_to_wait = None;
        match_object_panic!(arg.cv,"MessageBarrier",value,
			"traffic" => traffic=Some(new_traffic(TrafficBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
			"tasks" | "servers" => tasks=Some(value.as_usize().expect("bad value for tasks")),
			"messages_per_task_to_wait" => messages_per_task_to_wait=Some(value.as_usize().expect("bad value for messages_per_task_to_wait")),
			"expected_messages_to_consume_to_wait" => expected_messages_to_consume_to_wait=Some(value.as_usize().expect("bad value for expected_messages_to_consume_to_wait")),
		);
        let tasks=tasks.expect("There were no tasks");
        let traffic=traffic.expect("There were no traffic");
        let messages_per_task_to_wait=messages_per_task_to_wait.expect("There were no messages_per_task_to_wait");

        if traffic.number_tasks() != tasks {
            panic!("The number of tasks in the traffic and the number of tasks in the barrier are different.");
        }

        MessageBarrier {
            tasks,
            traffic,
            messages_per_task_to_wait,
            total_sent_per_task: vec![0; tasks],
            total_sent: 0,
            total_consumed: 0,
            total_consumed_per_task: vec![0; tasks],
            expected_messages_to_consume_to_wait,
        }
    }
}

#[allow(dead_code)]
pub struct BuildMessageBarrierCVArgs {
    pub traffic: ConfigurationValue,
    pub tasks: usize,
    pub messages_per_task_to_wait: usize,
    pub expected_messages_to_consume_to_wait: Option<usize>,
}

#[allow(dead_code)]
pub fn build_message_barrier_cv(args: BuildMessageBarrierCVArgs) -> ConfigurationValue
{
    let mut cv = vec![
        ("traffic".to_string(), args.traffic),
        ("tasks".to_string(), ConfigurationValue::Number(args.tasks as f64)),
        ("messages_per_task_to_wait".to_string(), ConfigurationValue::Number(args.messages_per_task_to_wait as f64)),
    ];

    if let Some(expected_messages_to_consume_to_wait) = args.expected_messages_to_consume_to_wait {
        cv.push(("expected_messages_to_consume_to_wait".to_string(), ConfigurationValue::Number(expected_messages_to_consume_to_wait as f64)));
    }

    ConfigurationValue::Object("MessageBarrier".to_string(), cv)
}


/**
MPI collectives implementations based on TrafficCredit

```ignore
AllGather{
    tasks: 64,
    data_size: 1000, //The total data size to all-gather. Each task starts with a data slice of size data_size/tasks.
    algorithm: Hypercube{
        neighbours_order: [32, 16, 8, 4, 2, 1], //Optional, the order to iter hypercube neighbours
    },
}
AllGather{
    tasks: 64,
    data_size: 1000, //The total data size to all-gather. Each task starts with a data slice of size data_size/tasks.
    algorithm: Ring,
}

ScatterReduce{
    tasks: 64,
    data_size: 1000, //The total data size to scatter-reduce. Each task ends with a data slice reduced of size data_size/tasks.
    algorithm: Hypercube, //natural order iterating
}

AllReduce{
    tasks: 64,
    data_size: 1000, //The total data size to all-reduce.
    algorithm: Hypercube, //natural order iterating
}

All2All{
    tasks: 64,
    data_size: 1000, //The total data size to all2all. Each task sends a data slice of size data_size/tasks to all the other tasks.
    rounds: 2, //Optional, the number of rounds to send all the data.
}
```
 **/

#[derive(Debug)]
pub enum MPICollectiveAlgorithm
{
    Hypercube(Option<ConfigurationValue>), //order in which to iterate the Hypercube neighbours
    Ring,
    // Optimal(Option<ConfigurationValue>),
}

fn parse_algorithm_from_cv(configuration_value: &ConfigurationValue) -> MPICollectiveAlgorithm
{
    if let ConfigurationValue::Object(ref cv, _) = configuration_value
    {
        let mut algorithm = None;
        match cv.as_str() {
            "Hypercube" => {
                let mut neighbours_order = None;
                match_object_panic!(configuration_value,"Hypercube",value,
                "neighbours_order" => neighbours_order = Some(value.clone()),
            );
                algorithm = Some(MPICollectiveAlgorithm::Hypercube(neighbours_order));
            },
            "Ring" => algorithm = Some(MPICollectiveAlgorithm::Ring),
            _ => {}
        }
        algorithm.expect("There should be a valid algorithm")

    } else {

        panic!("The algorithm should be an object");
    }
}

#[derive(Quantifiable)]
#[derive(Debug)]
pub struct MPICollective {}

impl MPICollective
{
    pub fn new(traffic: String, mut arg:TrafficBuilderArgument) ->  Box<dyn Traffic>
    {
        let traffic_cv = match traffic.as_str() {
            "ScatterReduce" =>{
                let mut tasks = None;
                let mut data_size = None;
                let mut algorithm = MPICollectiveAlgorithm::Hypercube(None);
                match_object_panic!(arg.cv,"ScatterReduce",value,
					"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
					"algorithm" => algorithm = parse_algorithm_from_cv(value),
					"data_size" => data_size = Some(value.as_f64().expect("bad value for data_size") as usize),
				);

                match algorithm {
                    MPICollectiveAlgorithm::Hypercube(_) => Some(get_scatter_reduce_hypercube(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"))),
                    MPICollectiveAlgorithm::Ring => Some(ring_iteration(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"), 1)),
                    // _ => panic!("Unknown algorithm: {:?}", algorithm),
                }
            },
            "AllGather" =>{
                let mut tasks = None;
                let mut data_size = None;
                let mut algorithm =  MPICollectiveAlgorithm::Hypercube(None);
                match_object_panic!(arg.cv,"AllGather",value,
					"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
					"algorithm" => algorithm = parse_algorithm_from_cv(value),
					"data_size" => data_size = Some(value.as_f64().expect("bad value for data_size") as usize),
				);
                match algorithm {
                    MPICollectiveAlgorithm::Hypercube(neighbours_order) => Some(get_all_gather_hypercube(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"), neighbours_order.as_ref())),
                    MPICollectiveAlgorithm::Ring => Some(ring_iteration(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"), 1)),
                    // _ => panic!("Unknown algorithm: {:?}", algorithm),
                }
            },
            "AllReduce" =>{
                let mut tasks = None;
                let mut data_size = None;
                let mut algorithm = MPICollectiveAlgorithm::Hypercube(None);
                match_object_panic!(arg.cv,"AllReduce",value,
					"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
					"algorithm" => algorithm = parse_algorithm_from_cv(value),
					"data_size" => data_size = Some(value.as_f64().expect("bad value for data_size") as usize),
				);

                match algorithm {
                    MPICollectiveAlgorithm::Hypercube(neighbours_order) => Some(get_all_reduce_optimal(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"), neighbours_order.as_ref())),
                    MPICollectiveAlgorithm::Ring => Some(get_all_reduce_ring(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"))),
                    // _ => panic!("Unknown algorithm: {}", algorithm),
                }
            },
            "All2All" =>{
                let mut tasks = None;
                let mut data_size = None;
                let mut rounds = 1;
                match_object_panic!(arg.cv,"All2All",value,
					"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
					"data_size" => data_size = Some(value.as_f64().expect("bad value for data_size") as usize),
                    "rounds" => rounds = value.as_usize().expect("bad value for rounds") as usize,
				);

                Some(get_all2all(tasks.expect("There were no tasks"), data_size.expect("There were no data_size"), rounds))
            },

            _ => panic!("Unknown traffic type: {}", traffic),
        };

        new_traffic(TrafficBuilderArgument{cv:&traffic_cv.expect("There should be a CV"),rng:&mut arg.rng,..arg})
    }
}

//Scater-reduce or all-gather in a ring
fn ring_iteration(tasks: usize, data_size: usize, iterations: usize) -> ConfigurationValue {

    let message_size = data_size/tasks;

    let neighbours_vector = vec![vec![1]];
    let immediate_neighbours_builder = ImmediateNeighboursCVBuilder {
        sides: vec![tasks],
        vector_neighbours: neighbours_vector,
        modular: true,
    };
    let immediate_neighbours = immediate_neighbours_cv_builder(immediate_neighbours_builder);

    let message_to_vector_builder = SendMessageToVectorCVBuilder{
        tasks,
        one_to_many_pattern: immediate_neighbours,
        message_size,
        rounds: (tasks -1) * iterations,
    };
    let message_to_vector = build_send_message_to_vector_cv(message_to_vector_builder);

    let initial_credits = get_candidates_selection(
        ConfigurationValue::Object("Identity".to_string(), vec![]),
        tasks,
    );
    let traffic_manager_builder = BuildTrafficManagerCVArgs{
        tasks,
        credits_to_activate: 1,
        messages_per_transition: 1,
        credits_per_received_message: 1,
        traffic: message_to_vector,
        initial_credits,
    };
    let traffic_manager = get_traffic_manager(traffic_manager_builder);

    traffic_manager
}


fn get_scatter_reduce_hypercube(tasks: usize, data_size: usize) -> ConfigurationValue
{
    //log2 the number of tasks and panic if its not a power of 2
    let messages = (tasks as f64).log2().round() as usize;
    if 2usize.pow(messages as u32) != tasks
    {
        panic!("The number of tasks must be a power of 2");
    }

    let hypercube_neighbours = ConfigurationValue::Object("HypercubeNeighbours".to_string(), vec![]);

    let send_message_to_vector_cv_builder = SendMessageToVectorCVBuilder{
        tasks,
        one_to_many_pattern: hypercube_neighbours,
        message_size: 0, //IDK if this is correct
        rounds: 1, //only send one time to the neighbours
    };
    let send_message_to_vector_cv = build_send_message_to_vector_cv(send_message_to_vector_cv_builder);

    let candidates_selection = get_candidates_selection(
        ConfigurationValue::Object("Identity".to_string(), vec![]),
        tasks,
    );
    let traffic_manager_builder = BuildTrafficManagerCVArgs{
        tasks,
        credits_to_activate: 1,
        messages_per_transition: 1,
        credits_per_received_message: 1,
        traffic: send_message_to_vector_cv,
        initial_credits: candidates_selection,
    };
    let traffic_manager = get_traffic_manager(traffic_manager_builder);


    //Now list dividing the data size by to in each iteration till number of messages
    let messages_sizes = (1..=messages).map(|i| data_size / 2usize.pow(i as u32) ).collect::<Vec<_>>();

    let message_size_mod = BuildMessageSizeModifierCVArgs{
        tasks,
        traffic: traffic_manager,
        message_sizes: messages_sizes,
    };

    get_message_size_modifier(message_size_mod)

}

fn get_all_gather_hypercube(tasks: usize, data_size: usize, _neighbours_order: Option<&ConfigurationValue>) -> ConfigurationValue
{
    //log2 the number of tasks and panic if its not a power of 2
    let messages = (tasks as f64).log2().round() as usize;
    if 2usize.pow(messages as u32) != tasks
    {
        panic!("The number of tasks must be a power of 2");
    }

    let hypercube_neighbours = ConfigurationValue::Object("HypercubeNeighbours".to_string(), vec![]);

    let send_message_to_vector_cv_builder = SendMessageToVectorCVBuilder{
        tasks,
        one_to_many_pattern: hypercube_neighbours,
        message_size: 0, //IDK if this is correct
        rounds: 1,
    };
    let send_message_to_vector_cv = build_send_message_to_vector_cv(send_message_to_vector_cv_builder);

    let candidates_selection = get_candidates_selection(
        ConfigurationValue::Object("Identity".to_string(), vec![]),
        tasks,
    );
    let traffic_manager_builder = BuildTrafficManagerCVArgs{
        tasks,
        credits_to_activate: 1,
        messages_per_transition: 1,
        credits_per_received_message: 1,
        traffic: send_message_to_vector_cv,
        initial_credits: candidates_selection,
    };
    let traffic_manager = get_traffic_manager(traffic_manager_builder);


    //Now list dividing the data size by to in each iteration till number of messages
    let messages_sizes = (1..=messages).map(|i| data_size / 2usize.pow(i as u32) ).rev().collect::<Vec<_>>();

    let message_size_mod = BuildMessageSizeModifierCVArgs{
        tasks,
        traffic: traffic_manager,
        message_sizes: messages_sizes,
    };

    get_message_size_modifier(message_size_mod)
}

fn get_all_reduce_optimal(tasks: usize, data_size: usize, neighbours_order: Option<&ConfigurationValue>) -> ConfigurationValue
{
    let scatter_reduce_hypercube = get_scatter_reduce_hypercube(tasks, data_size);
    let all_gather_hypercube = get_all_gather_hypercube(tasks, data_size, neighbours_order);

    let messages_per_task = (tasks as f64).log2().round() as usize;
    let traffic_message_task_sequence_args = BuilderMessageTaskSequenceCVArgs{
        tasks,
        traffics: vec![scatter_reduce_hypercube, all_gather_hypercube],
        messages_to_send_per_traffic: vec![messages_per_task, messages_per_task],
        messages_to_consume_per_traffic: Some(vec![messages_per_task, messages_per_task]),
    };
    get_traffic_message_task_sequence(traffic_message_task_sequence_args)
}

fn get_all_reduce_ring(tasks: usize, data_size: usize) -> ConfigurationValue
{
    ring_iteration(tasks, data_size, 2)
}

pub(crate) fn get_all2all(tasks: usize, data_size: usize, rounds: usize) -> ConfigurationValue
{
    let total_messages = (tasks -1) * rounds;
    if rounds == 0 {
        panic!("The number of rounds must be greater than 0");
    }
    let message_size = (data_size/tasks)/rounds;

    let hypercube_neighbours = ConfigurationValue::Object("AllNeighbours".to_string(), vec![]);
    let send_message_to_vector_cv_builder = SendMessageToVectorCVBuilder{
        tasks,
        one_to_many_pattern: hypercube_neighbours,
        message_size,
        rounds,
    };
    let send_message_to_vector_cv = build_send_message_to_vector_cv(send_message_to_vector_cv_builder);

    let candidates_selection = get_candidates_selection(
        ConfigurationValue::Object("Identity".to_string(), vec![]),
        tasks,
    );
    let traffic_manager_builder = BuildTrafficManagerCVArgs{
        tasks,
        credits_to_activate: 1,
        messages_per_transition: total_messages,
        credits_per_received_message: 0,
        traffic: send_message_to_vector_cv,
        initial_credits: candidates_selection,
    };
    get_traffic_manager(traffic_manager_builder)
}

#[cfg(test)]
mod tests {
    use rand::prelude::StdRng;
    use rand::SeedableRng;
    use crate::Plugs;
    use crate::traffic::collectives::{get_all2all, get_all_reduce_optimal, get_all_reduce_ring};
    use crate::traffic::new_traffic;

    #[test]
    fn test_allreduce_optimal() {
        let tasks = 64;
        let data_size = 128;
        let cv = get_all_reduce_optimal(tasks, data_size, None);
        let mut rng = StdRng::seed_from_u64(0);
        println!("All reduce optimal");
        println!("{}", cv.format_terminal());

        let traffic_builder = super::TrafficBuilderArgument {
            cv: &cv,
            rng: &mut rng,
            plugs: &Plugs::default(),
            topology: None,
        };
        let mut t = new_traffic(traffic_builder);

        let iterations = 6; //6 +6
        assert_eq!(t.number_tasks(), tasks);
        for iteration in 0..iterations // extract all messages for reduce-scatter
        {
            let mut messages = vec![];

            for i in 0..tasks {
                assert_eq!(t.should_generate(i, 0, &mut rng), true); //inserting all2all
            }

            for i in 0..tasks {
                let message = t.generate_message(i, 0, None, &mut rng).unwrap();
                assert_eq!(message.size, data_size /(2 << iteration));
                messages.push(message);
            }

            //now check that tasks are done and their state is FinishedGenerating. also traffic is not finished.
            for i in 0..tasks {
                assert_eq!(t.should_generate(i, 0, &mut rng), false);
            }

            for i in 0..messages.len() {
                assert_eq!(t.consume(messages[i].origin, &*messages[i], 0, None, &mut rng), true);
            }
        }

        assert_eq!(t.is_finished(Some(&mut rng)), false);

        for iteration in 0..iterations // extract all messages for all-gather
        {
            let mut messages = vec![];

            for i in 0..tasks {
                assert_eq!(t.should_generate(i, 0, &mut rng), true); //inserting all2all
            }

            for i in 0..tasks {
                let message = t.generate_message(i, 0, None, &mut rng).unwrap();
                assert_eq!(message.size, data_size /(2 << (iterations - iteration - 1)));
                messages.push(message);
            }

            //now check that tasks are done and their state is FinishedGenerating. also traffic is not finished.
            for i in 0..tasks {
                assert_eq!(t.should_generate(i, 0, &mut rng), false);
            }

            for i in 0..messages.len() {
                assert_eq!(t.consume(messages[i].origin, &*messages[i], 0, None, &mut rng), true);
            }
        }
        assert_eq!(t.is_finished(Some(&mut rng)), true);
    }

    #[test]
    fn test_allreduce_ring() {
        let tasks = 64;
        let data_size = 128;
        let cv = get_all_reduce_ring(tasks, data_size);
        let mut rng = StdRng::seed_from_u64(0);
        println!("All reduce ring");
        println!("{}", cv.format_terminal());

        let traffic_builder = super::TrafficBuilderArgument {
            cv: &cv,
            rng: &mut rng,
            plugs: &Plugs::default(),
            topology: None,
        };

        let mut t = new_traffic(traffic_builder);
        let iterations = tasks -1;
        assert_eq!(t.number_tasks(), tasks);
        for iteration in 0..iterations // extract all messages for reduce-scatter
        {
            let mut messages = vec![];

            for i in 0..tasks {
                assert_eq!(t.should_generate(i, 0, &mut rng), true); //inserting all2all
            }

            for i in 0..tasks {
                let message = t.generate_message(i, 0, None, &mut rng).unwrap();
                assert_eq!(message.size, data_size / (tasks) );
                messages.push(message);
            }

            //now check that tasks are done and their state is FinishedGenerating. also traffic is not finished.
            for i in 0..tasks {
                assert_eq!(t.should_generate(i, 0, &mut rng), false);
            }

            for i in 0..messages.len() {
                assert_eq!(t.consume(messages[i].origin, &*messages[i], 0, None, &mut rng), true);
            }
        }

        assert_eq!(t.is_finished(Some(&mut rng)), false);

        for iteration in 0..iterations // extract all messages for reduce-scatter
        {
            let mut messages = vec![];

            for i in 0..tasks {
                assert_eq!(t.should_generate(i, 0, &mut rng), true); //inserting all2all
            }

            for i in 0..tasks {
                let message = t.generate_message(i, 0, None, &mut rng).unwrap();
                assert_eq!(message.size, data_size / (tasks) );
                messages.push(message);
            }

            //now check that tasks are done and their state is FinishedGenerating. also traffic is not finished.
            for i in 0..tasks {
                assert_eq!(t.should_generate(i, 0, &mut rng), false);
            }

            for i in 0..messages.len() {
                assert_eq!(t.consume(messages[i].origin, &*messages[i], 0, None, &mut rng), true);
            }
        }
        assert_eq!(t.is_finished(Some(&mut rng)), true);
    }

    #[test]
    fn test_all2all() {
        let cv = get_all2all(64, 128, 2);
        println!("All2All");
        println!("{}", cv.format_terminal());
    }
}

