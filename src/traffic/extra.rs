use std::cmp;
use std::collections::{HashSet, VecDeque};
use crate::general_pattern::pattern::{get_cartesian_transform_from_builder, get_composition_pattern_cv, get_linear_transform, BuildCartesianTransformCV, BuildCompositionCV, BuildLinearTransformCV};
use std::convert::TryInto;
use std::rc::Rc;
use quantifiable_derive::Quantifiable;

use rand::prelude::StdRng;
use crate::{match_object_panic, Message, Time};

use crate::AsMessage;
use crate::config_parser::ConfigurationValue;
use crate::general_pattern::{new_one_to_many_pattern, new_pattern, GeneralPatternBuilderArgument};
use crate::general_pattern::many_to_many_pattern::{new_many_to_many_pattern, ManyToManyParam, ManyToManyPattern};
use crate::general_pattern::one_to_many_pattern::neighbours::{get_king_neighbours_cv, immediate_neighbours_cv_builder, ImmediateNeighboursCVBuilder};
use crate::general_pattern::one_to_many_pattern::OneToManyPattern;
use crate::general_pattern::pattern::extra::{get_candidates_selection, get_cartesian_transform, get_hotspot_destination};
use crate::general_pattern::prelude::Pattern;
use crate::packet::ReferredPayload;
use crate::topology::cartesian::CartesianData;
use crate::topology::Topology;
use crate::traffic::{build_traffic_map_cv, build_traffic_sum_cv, new_traffic, BuildTrafficMapCVArgs, BuildTrafficSumCVArgs, TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};
use crate::traffic::basic::{build_send_message_to_vector_cv, SendMessageToVectorCVBuilder};
use crate::measures::TrafficStatistics;
use crate::traffic::TaskTrafficState::{FinishedGenerating, Generating};

/**
Traffic which allows a task from a traffic to generate a message when it has enough credits.
After generating the message, 'credits_to_activate' credits are consumed.
A task gains credits when it consumes messages, and each task has set an initial number of credits.
```ignore
TrafficManager{ //Step by step All2All
	tasks: 16,
	traffic: All2All{...},
	credits_to_activate: 1,
	credits_per_received_message: 1,
	messages_per_transition: 1,
	initial_credits: Identity{},
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct TrafficManager
{
	///Number of tasks applying this traffic.
	tasks: usize,
	///The SimplePattern of the communication.
	traffic: Box<dyn Traffic>,
	///Credits needed to activate the transition
	credits_to_activate:usize,
	///Credit count per origin
	credits: Vec<usize>,
	///The credits to sum when a message is received
	credits_per_received_message:usize,
	///Messages per transition
	messages_per_transition:usize,
	///The number of messages each task has pending to sent.
	available_messages_to_send: Vec<usize>,
	///Number of messages in-flight.
	generated_messages_count: usize,
	next_id: u128,
}

impl Traffic for TrafficManager
{
	fn generate_message(&mut self, origin:usize, cycle:Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
	{
		if origin>=self.tasks
		{
			panic!("origin {} does not belong to the traffic",origin);
		}
		if self.available_messages_to_send[origin] == 0
		{
			panic!("origin {} has no pending messages",origin);
		}
		self.available_messages_to_send[origin]-=1;
		let message = self.traffic.generate_message(origin, cycle, topology, rng)?;
		let id = u128::from_le_bytes(message.payload()[0..16].try_into().expect("bad payload"));
		self.generated_messages_count += 1;
		let mut payload = Vec::with_capacity(message.payload().len() + 4);
		payload.extend_from_slice(&id.to_le_bytes());
		payload.extend_from_slice(message.payload());
		let message = Rc::new(Message {
			origin,
			destination: message.destination,
			size: message.size,
			creation_cycle: message.creation_cycle,
			payload,
			id_traffic: message.id_traffic,
		});

		self.next_id += 1;
		Ok(message)
	}
	fn probability_per_cycle(&self, task:usize) -> f32
	{
		if self.available_messages_to_send[task]>0 && self.traffic.probability_per_cycle(task) > 0.0
		{
			1.0
		}
		else
		{
			0.0
		}
	}

	fn should_generate( &mut self, task:usize, cycle:Time, rng: &mut StdRng) -> bool
	{
		if self.credits[task] >= self.credits_to_activate
		{
			self.available_messages_to_send[task] += self.messages_per_transition;
			self.credits[task] -= self.credits_to_activate;
		}
		self.available_messages_to_send[task] > 0 && self.traffic.should_generate(task, cycle, rng)
	}

	fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> bool
	{
		self.credits[task] += self.credits_per_received_message;
		let id = u128::from_le_bytes(message.payload()[0..16].try_into().expect("bad payload"));
		let sub_message_payload = &message.payload()[16..];
		let mut inner_message = ReferredPayload::from(message);
		inner_message.payload = sub_message_payload;
		if  inner_message.destination != task
		{
			panic!("Message {} was not sent to task {}",id,task);
		}
		if self.generated_messages_count == 0{
			panic!("This traffic shouldnt consume any message.")
		}
		self.generated_messages_count -= 1;
		self.traffic.consume(task, &inner_message, cycle, topology, rng)
	}
	fn is_finished(&mut self, rng: Option<&mut StdRng>) -> bool
	{
		if self.traffic.is_finished(rng) {
			return true;
		}

		if self.generated_messages_count > 0 //messages traveling through the network
		{
			return false;
		}

		if self.available_messages_to_send.iter().sum::<usize>() > 0 //messages waiting to be sent
		{
			return false;
		}

		//if there is a task with enough credits to activate, then it is not finished
		for &c in self.credits.iter()
		{
			if c >= self.credits_to_activate
			{
				return false;
			}
		}
		panic!("TrafficManager is not able to finish the underlying traffic");
	}
	fn task_state(&mut self, task:usize, cycle:Time) -> Option<TaskTrafficState>
	{
		self.traffic.task_state(task, cycle)
	}

	fn number_tasks(&self) -> usize {
		self.tasks
	}
}

impl TrafficManager
{
	pub fn new(arg:TrafficBuilderArgument) -> TrafficManager
	{
		let mut tasks=None;
		let mut subtraffic =None;
		let mut credits_to_activate=None;
		let mut credits_per_received_message=None;
		let mut messages_per_transition=None;
		let mut initial_credits=None;

		match_object_panic!(arg.cv,"TrafficManager",value,
			"traffic" => subtraffic=Some(new_traffic(TrafficBuilderArgument{cv:value, plugs:arg.plugs, topology:arg.topology, rng:arg.rng})),
			"tasks" | "servers" => tasks=Some(value.as_usize().expect("bad value for tasks")),
			"credits_to_activate" => credits_to_activate=Some(value.as_usize().expect("bad value for credits_to_activate")),
			"credits_per_received_message" => credits_per_received_message=Some(value.as_usize().expect("bad value for credits_per_received_message")),
			"messages_per_transition" => messages_per_transition=Some(value.as_usize().expect("bad value for messages_per_transition")),
			"initial_credits" => initial_credits=Some(new_pattern(GeneralPatternBuilderArgument{cv:value,plugs:arg.plugs})),
		);

		let tasks=tasks.expect("There were no tasks");
		let subtraffic = subtraffic.expect("There were no subtraffic");
		let credits_to_activate=credits_to_activate.expect("There were no credits_to_activate");
		let credits_per_received_message=credits_per_received_message.expect("There were no credits_per_received_message");
		let messages_per_transition=messages_per_transition.expect("There were no messages_per_transition");
		let mut initial_credits=initial_credits.expect("There were no initial_credits");

		let available_messages_to_send = vec![0;tasks];
		initial_credits.initialize(tasks, tasks, arg.topology, arg.rng);
		let credits = (0..tasks).map(|i| initial_credits.get_destination(i, arg.topology, arg.rng)).collect::<Vec<usize>>();

		TrafficManager {
			tasks,
			traffic: subtraffic,
			credits_to_activate,
			credits,
			credits_per_received_message,
			messages_per_transition,
			available_messages_to_send,
			generated_messages_count: 0,
			next_id: 0,
		}
	}
}

//TrafficManager CV builder
pub struct BuildTrafficManagerCVArgs{
	pub tasks: usize,
	pub credits_to_activate:usize,
	pub messages_per_transition: usize,
	pub credits_per_received_message: usize,
	pub traffic: ConfigurationValue,
	pub initial_credits: ConfigurationValue,
}

pub fn get_traffic_manager(args: BuildTrafficManagerCVArgs) -> ConfigurationValue
{
	let arg_vec = vec![
		("tasks".to_string(), ConfigurationValue::Number(args.tasks as f64)),
		("credits_to_activate".to_string(), ConfigurationValue::Number(args.credits_to_activate as f64)),
		("messages_per_transition".to_string(), ConfigurationValue::Number(args.messages_per_transition as f64)),
		("credits_per_received_message".to_string(), ConfigurationValue::Number(args.credits_per_received_message as f64)),
		("traffic".to_string(), args.traffic),
		("initial_credits".to_string(), args.initial_credits),
	];

	ConfigurationValue::Object("TrafficManager".to_string(), arg_vec)
}


/**
Traffic which modifies the size of the messages sent by a task.
```ignore
MessageSizeModifier{
	tasks: 7,
	traffic: All2All{...},
	message_sizes: [16, 32, 64, 128, 256, 512], //6 different sizes of messages
}
```
 **/
#[derive(Debug, Quantifiable)]
pub struct MessageSizeModifier {
	/// The traffic to modify
	traffic: Box<dyn Traffic>,
	/// id of the message
	id_next: u128,
	/// Hashmap with the original size of the messages
	original_size: std::collections::HashMap<u128, usize>,
	/// Vector with the size of the messages
	message_sizes:Vec<usize>,
	/// Vector with the messages sent by each task
	messages_sent: Vec<usize>,
}

impl Traffic for MessageSizeModifier {
	fn generate_message(&mut self, origin:usize, cycle:Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
	{
		let message = self.traffic.generate_message(origin, cycle, topology, rng)?;
		let mut payload = Vec::with_capacity(message.payload().len() + 4);
		payload.extend_from_slice(&self.id_next.to_le_bytes());
		payload.extend_from_slice(message.payload());
		if self.messages_sent[origin] >= self.message_sizes.len()
		{
			panic!("MessageSizeModifier: origin {} has no more messages to send",origin);
		}
		let message = Rc::new(Message {
			origin,
			destination: message.destination,
			size: self.message_sizes[self.messages_sent[origin]],
			creation_cycle: message.creation_cycle,
			payload,
			id_traffic: message.id_traffic,
		});
		self.original_size.insert(self.id_next, message.size);
		self.messages_sent[origin] += 1;
		self.id_next += 1;
		Ok(message)
	}
	fn probability_per_cycle(&self, task:usize) -> f32
	{
		self.traffic.probability_per_cycle(task)
	}
	fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> bool
	{
		let id = u128::from_le_bytes(message.payload()[0..16].try_into().expect("bad payload"));
		let sub_message_payload = &message.payload()[16..];
		let original_size = self.original_size.remove(&id).expect("MessageSizeModifier: message not found");
		let mut inner_message = ReferredPayload::from(message);
		inner_message.payload = sub_message_payload;
		inner_message.size = original_size;
		self.traffic.consume(task, &inner_message, cycle, topology, rng)
	}
	fn is_finished(&mut self, rng: Option<&mut StdRng>) -> bool
	{
		self.traffic.is_finished(rng)
	}
	fn should_generate( &mut self, task:usize, cycle:Time, rng: &mut StdRng) -> bool
	{
		self.traffic.should_generate(task, cycle, rng)
	}
	fn task_state(&mut self, task:usize, cycle:Time) -> Option<TaskTrafficState>
	{
		self.traffic.task_state(task, cycle)
	}
	fn number_tasks(&self) -> usize {
		self.traffic.number_tasks()
	}
}

impl MessageSizeModifier {
	pub fn new(arg:TrafficBuilderArgument) -> MessageSizeModifier
	{
		let mut traffic = None;
		let mut message_sizes = None;
		let mut tasks = None;

		match_object_panic!(arg.cv,"MessageSizeModifier",value,
			"tasks" | "servers" => tasks=Some(value.as_usize().expect("bad value for tasks")),
			"traffic" => traffic=Some(new_traffic(TrafficBuilderArgument{cv:value, plugs:arg.plugs, topology:arg.topology, rng:arg.rng})),
			"message_sizes" => message_sizes=Some(value.as_array().expect("bad value for size").iter().map(|v| v.as_usize().expect("bad value for size")).collect()),
		);

		let tasks = tasks.expect("There were no tasks");
		let traffic = traffic.expect("There were no traffic");
		let size = message_sizes.expect("There were no size");

		MessageSizeModifier {
			traffic,
			id_next: 0,
			original_size: std::collections::HashMap::new(),
			message_sizes: size,
			messages_sent: vec![0; tasks],
		}
	}
}

pub struct BuildMessageSizeModifierCVArgs{
	pub tasks: usize,
	pub traffic: ConfigurationValue,
	pub message_sizes: Vec<usize>,
}

pub fn get_message_size_modifier(args: BuildMessageSizeModifierCVArgs) -> ConfigurationValue
{
	let arg_vec = vec![
		("tasks".to_string(), ConfigurationValue::Number(args.tasks as f64)),
		("traffic".to_string(), args.traffic),
		("message_sizes".to_string(), ConfigurationValue::Array(args.message_sizes.iter().map(|&x| ConfigurationValue::Number(x as f64)).collect())),
	];

	ConfigurationValue::Object("MessageSizeModifier".to_string(), arg_vec)
}


/**
Traffic which collects statistics of a Traffic below. It doesnt do anything else
```ignore
TrafficStatistics{
	traffic: All2All{...},
}
```
 **/

#[derive(Debug, Quantifiable)]
pub struct StatisticsCollector {
	traffic: Box<dyn Traffic>,
	statistics: TrafficStatistics,
}

impl Traffic for StatisticsCollector {
    fn generate_message(&mut self, origin:usize, cycle:Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError> {
        let result = self.traffic.generate_message(origin, cycle, topology, rng);

        if let Ok(message) = &result {
            self.statistics.track_created_message(cycle, message.size);
        }
        result
    }

    fn probability_per_cycle(&self, task:usize) -> f32 {
        self.traffic.probability_per_cycle(task)
    }

    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> bool {
        let consumed = self.traffic.consume(task, message, cycle, topology, rng);
        if consumed {
            self.statistics.track_consumed_message(message.origin(), task, cycle, cycle - message.creation_cycle(), message.size());
        }
        consumed
    }

    fn is_finished(&mut self, rng: Option<&mut StdRng>) -> bool {
        self.traffic.is_finished(rng)
    }

    fn should_generate(&mut self, task:usize, cycle:Time, rng: &mut StdRng) -> bool {
        let gen = self.traffic.should_generate(task, cycle, rng);
		if gen{
			self.statistics.track_task_state(task, TaskTrafficState::Generating, cycle);
		}else { 
			self.statistics.track_task_state(task, TaskTrafficState::UnspecifiedWait, cycle);
		}
		gen
    }

    fn task_state(&mut self, task:usize, cycle:Time) -> Option<TaskTrafficState> {
        self.traffic.task_state(task, cycle)
    }

    fn number_tasks(&self) -> usize {
        self.traffic.number_tasks()
    }

    fn get_statistics(&self) -> Option<TrafficStatistics> {
        Some(self.statistics.clone())
    }
}

impl StatisticsCollector {
    pub fn new(arg: TrafficBuilderArgument) -> StatisticsCollector {
        let mut traffic = None;
        let mut statistics_temporal_step = None;
        let mut box_size = None;

        match_object_panic!(arg.cv, "StatisticsCollector", value,
            "traffic" => traffic = Some(new_traffic(TrafficBuilderArgument{cv:value, plugs:arg.plugs, topology:arg.topology, rng:arg.rng})),
            "temporal_step" => statistics_temporal_step = Some(value.as_time().expect("bad value for statistics_temporal_step")),
            "box_size" => box_size = Some(value.as_usize().expect("bad value for box_size")),
        );

        let traffic = traffic.expect("Traffic is required for StatisticsCollector");
        let number_tasks = traffic.number_tasks();
        let statistics = TrafficStatistics::new(
			number_tasks,
	        statistics_temporal_step.expect("statistics_temporal_step is required for StatisticsCollector"),
	        box_size.expect("box_size is required for StatisticsCollector"),
		);

        StatisticsCollector {
            traffic,
            statistics,
        }
    }
}

pub struct BuildStatisticsCollectorCVArgs {
    pub traffic: ConfigurationValue,
    pub temporal_step: Time,
    pub box_size: usize,
}

pub fn get_statistics_collector_cv(args: BuildStatisticsCollectorCVArgs) -> ConfigurationValue {
    let arg_vec = vec![
        ("traffic".to_string(), args.traffic),
        ("temporal_step".to_string(), ConfigurationValue::Number(args.temporal_step as f64)),
        ("box_size".to_string(), ConfigurationValue::Number(args.box_size as f64))
    ];
    ConfigurationValue::Object("StatisticsCollector".to_string(), arg_vec)
}

#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Block{
	id: Option<usize>,
	coordinates: Vec<usize>,
	children: Vec<Block>,
	level: usize,
}

impl Block{

	fn get_ordered_children_coordinates_from_block_coord(b_coord: &Vec<usize>) -> Vec<Vec<usize>>{
		let sub_space_children_cartesian_data = CartesianData::new(&vec![2; b_coord.len()]);
		let children_coord_start = b_coord.clone().into_iter().map(|x| x * 2).collect::<Vec<usize>>();
		let mut final_vector_of_children = vec![];
		for j in 0..sub_space_children_cartesian_data.size{
			let local_vector = sub_space_children_cartesian_data.unpack(j);
			final_vector_of_children.push(children_coord_start.clone().into_iter().zip(local_vector.iter()).map(|(a, b)| a + b).collect::<Vec<usize>>());
		}
		final_vector_of_children
	}

	fn refine_block(&mut self){
		let mut children_blocks = vec![];
		let children_coordinates = Self::get_ordered_children_coordinates_from_block_coord(&self.coordinates);
		for j in 0..children_coordinates.len(){
			children_blocks.push(
				Block{
					id: Some(j),
					coordinates: children_coordinates[j].clone(),
					children: vec![],
					level: self.level + 1,
				}
			);
		}
		//order children by subblock_label
		// children_blocks.sort_by(|a, b| a.id.unwrap().cmp(&b.id.unwrap()));
		self.children = children_blocks;
		self.id = None;
	}

	fn refine_tree(&mut self, to_refine: &mut Vec<usize>, max_levels: usize, spaces_by_level:&Vec<Vec<usize>>){
		if to_refine.is_empty(){
			return;
		}
		if max_levels == self.level +1{ //dont refine more
			to_refine.retain(|&a| a != self.id.unwrap()); //So in case it needs to be removed its done
			return;
		}

		if let Some(id) = self.id{
			if self.children.len() != 0{
				panic!("Trying to refine a block that has not been refined yet, but it has children. This should not happen.")
			}
			if to_refine.contains(&id){
				//refine this block
				self.refine_block();
				to_refine.retain(|&a| a != id); //So in case it needs to be removed its done
			}
		}else {
			if self.children.len() == 0{
				panic!("Should have kids!")
			}
			for child in self.children.iter_mut() {
				child.refine_tree(to_refine, max_levels, spaces_by_level);
			}
		}
	}

	fn relabel_tree(&mut self, start_id_relabeling: usize)-> usize{
		if let Some(_) = self.id {
			if !self.children.is_empty() {
				panic!("Shouldnt have children")
			}
			self.id = Some(start_id_relabeling);
			start_id_relabeling + 1
		} else {
			if self.children.is_empty() {
				panic!("Should have children")
			}
			let mut next_id = start_id_relabeling;
			for child in self.children.iter_mut() {
				next_id = child.relabel_tree(next_id);
			}
			next_id
		}
	}

	pub fn refine_and_relabel_tree(&mut self, to_refine: &mut Vec<usize>, max_levels: usize, spaces_by_level:&Vec<Vec<usize>>, start_id_relabeling: usize) ->usize{
		if !to_refine.is_empty(){
			self.refine_tree(to_refine, max_levels, spaces_by_level);
		}
		self.relabel_tree(start_id_relabeling)
	}

	pub fn all_internal_meshblocks_list(&self) -> Vec<&Block>
	{
		let mut list = vec![];
		if let Some(_) = self.id{
			assert_eq!(self.children.len(), 0);
			list.push(self);
		}else {
			for child in self.children.iter(){
				list.extend(child.all_internal_meshblocks_list());
			}
		}
		list
	}
}

pub enum NeighbouringPattern {
	KingNeighbours,
	ManhattanNeighbours,
}

/**
Adaptive Mesh Refinement traffic kernel.
Simulates one step of the communications of a refined mesh.

## AMR (Adaptive Mesh Refinement)
```ignore
	AMR{
		tasks: 512,
		meshblock_space:[8, 8, 8], //1 meshblock per task to init
		max_levels: 3, //number of levels of refinement
		block_label: Identity{},
		neighbour_selection: KingNeighbours, // a NeigbouringPattern latter match with a pattern
		refinement_pattern: RandomFilter{...},
		message_size: 256, //between big neighbour blocks
	}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct AMR {
	tasks: usize,
	task_messages_to_send: Vec<VecDeque<(usize, usize)>>,
	meshblock_spaces_by_level: Vec<Vec<usize>>,
	meshblocks_roots: Vec<Block>,
	// all_meshblocks_list: &'a Vec<Block>,
	dimensions: usize,
	total_meshblocks: usize,
	max_levels: usize,
	meshblock_label: Box<dyn Pattern>,
	tasks_to_block: Vec<Vec<usize>>,
	block_to_task: Vec<usize>,
	neighbour_patterns: Vec<Box<dyn OneToManyPattern>>, //Pattern depending on the grid of refinement.
	refinement_pattern: Vec<Box<dyn ManyToManyPattern>>,
	// derefinement_pattern: Box<dyn ManyToManyPattern>,
	message_size: usize,
	messages_to_consume:usize,
	messages_consumed:usize,
	rng: StdRng,
}

impl Traffic for AMR {
	fn generate_message(&mut self, origin: usize, cycle: Time, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> Result<Rc<Message>, TrafficError> {
		let Some((destination, size)) = self.task_messages_to_send[origin].pop_front() else { panic!("Shouldn't be happening") };
		Ok(Rc::new(Message { origin, destination, size, creation_cycle: cycle, payload: vec![], id_traffic: None }))
	}

	fn probability_per_cycle(&self, task: usize) -> f32 {
		if self.task_messages_to_send[task].len() > 0{
			1.0
		}	else {
			0.0
		}
	}

	fn consume(&mut self, task: usize, message: &dyn AsMessage, _cycle: Time, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> bool {
		self.messages_consumed+=1;
		task == message.destination()
	}

	fn is_finished(&mut self, _rng: Option<&mut StdRng>) -> bool {
		self.messages_to_consume == self.messages_consumed
	}

	fn should_generate(&mut self, task: usize, _cycle: Time, _rng: &mut StdRng) -> bool {
		self.task_messages_to_send[task].len() > 0
	}

	fn task_state(&mut self, task: usize, _cycle: Time) -> Option<TaskTrafficState> {
		if self.task_messages_to_send[task].len() > 0{
			Some(Generating)
		}	else {
			Some(FinishedGenerating)
		}
	}

	fn number_tasks(&self) -> usize {
		self.tasks
	}

	fn get_statistics(&self) -> Option<TrafficStatistics> {
		None
	}
}

impl AMR {
	pub fn new(arg: TrafficBuilderArgument) -> AMR {
		let mut tasks = None;
		let mut meshblock_space = None;
		let mut max_levels = None;
		let mut meshblock_label = None;
		let mut neighbour_pattern = None;
		let mut refinement_pattern = None;
		let mut message_size = None;

		match_object_panic!(arg.cv,"AMR",value,
			"tasks" => tasks=Some(value.as_usize().expect("bad value for tasks")),
			"meshblock_space" => meshblock_space=Some(value.as_array().expect("bad value for meshblock_space").iter().map(|v| v.as_usize().expect("bad value for meshblock_space")).collect()),
			"max_levels" => max_levels=Some(value.as_usize().expect("bad value for max_levels")),
			"meshblock_label" => meshblock_label=Some(new_pattern(GeneralPatternBuilderArgument{cv:value, plugs:arg.plugs})),
			"neighbour_selection" => neighbour_pattern=Some(
				match value.as_str().expect("bad value for neighbour_pattern") {
					"KingNeighbours" => NeighbouringPattern::KingNeighbours,
					"ManhattanNeighbours" => NeighbouringPattern::ManhattanNeighbours,
					_ => panic!("bad value for neighbour_pattern"),
				}
			),
			"refinement_pattern" => refinement_pattern=Some(
				value.as_array().expect("bad value for meshblock_space").iter().map(|v| new_many_to_many_pattern(GeneralPatternBuilderArgument{cv:v, plugs:arg.plugs})).collect::<Vec<Box<dyn ManyToManyPattern>>>()
			),
			"message_size" => message_size=Some(value.as_usize().expect("bad value for message_size")),
		);
		let tasks = tasks.expect("tasks is required for AMRStep");
		let meshblock_space: Vec<usize> = meshblock_space.expect("meshblock_space is required for AMRStep");
		let dimensions = meshblock_space.len();
		let max_levels = max_levels.expect("max_levels is required for AMRStep");
		let mut meshblock_spaces_by_level: Vec<Vec<usize>> = vec![];
		let mut iter = meshblock_space.clone();
		for _ in 0..max_levels {
			meshblock_spaces_by_level.push(iter.clone());
			iter.iter_mut().for_each(|x| *x *= 2);
		}
		// let sub_meshblock_space = vec![2; dimensions];

		let mut neighbour_patterns: Vec<Box<dyn OneToManyPattern>> = vec![];
		match neighbour_pattern.expect("neighbour_pattern is required for AMRStep") {
			NeighbouringPattern::KingNeighbours => {
				for i in 0..max_levels {
					neighbour_patterns.push(
						new_one_to_many_pattern(GeneralPatternBuilderArgument {
							cv: &get_king_neighbours_cv(&meshblock_spaces_by_level[i], 1, true),
							plugs: arg.plugs
						})
					);
				}
			}
			NeighbouringPattern::ManhattanNeighbours => {
				panic!("ManhattanNeighbours is not implemented yet")
			}
		};
		let meshblocks_cartesian_data = CartesianData::new(&meshblock_space);
		let total_meshblocks = meshblocks_cartesian_data.size;
		for i in 0..neighbour_patterns.len() {
			neighbour_patterns[i].initialize(total_meshblocks * 2usize.pow(dimensions as u32).pow(i as u32), total_meshblocks * 2usize.pow(dimensions as u32).pow(i as u32), arg.topology, arg.rng);
		}

		let refinement_pattern = refinement_pattern.expect("refinement_pattern is required for AMRStep");
		let message_size = message_size.expect("message_size is required for AMRStep");
		let mut meshblock_label = meshblock_label.expect("meshblock_label is required for AMRStep");
		meshblock_label.initialize(total_meshblocks, total_meshblocks, arg.topology, arg.rng);


		let mut meshblocks_roots = vec![];
		for i in 0..total_meshblocks {
			let coordinates = meshblocks_cartesian_data.unpack(i);
			let block = Block {
				id: Some(meshblock_label.get_destination(i, None, arg.rng)),
				coordinates,
				children: vec![],
				level: 0,
			};
			meshblocks_roots.push(block);
		}
		//now sort the blocks by ID so ID 0 is first and goes incrementally
		meshblocks_roots.sort_by(|a, b| a.id.unwrap().cmp(&b.id.unwrap()));
		// let all_meshblocks_list = &meshblocks_roots;
		let task_messages_to_send = vec![VecDeque::new(); tasks];
		let tasks_to_block = vec![vec![]; tasks];

		let mut traffic = AMR {
			tasks,
			task_messages_to_send,
			meshblock_spaces_by_level,
			meshblocks_roots,
			// all_meshblocks_list,
			dimensions,
			total_meshblocks,
			max_levels,
			meshblock_label,
			tasks_to_block,
			block_to_task: vec![],
			neighbour_patterns,
			refinement_pattern,
			message_size,
			messages_to_consume: 0,
			messages_consumed: 0,
			rng: arg.rng.clone(),
		};
		traffic.refine_mesh_and_update_variables();
		traffic.assign_blocks_to_tasks();
		traffic.generate_amr_step_messages();
		//Print some stats about the number of blocks per task, size to transmit (min and max in both)
		println!("AMR initialized with {} tasks and {} total meshblocks. Each task has between {} and {} blocks, and will send messages between {} and {}.",
			traffic.tasks,
			traffic.total_meshblocks,
			traffic.tasks_to_block.iter().map(|b| b.len()).min().unwrap(),
			traffic.tasks_to_block.iter().map(|b| b.len()).max().unwrap(),
			traffic.task_messages_to_send.iter().map(|b| b.len()).min().unwrap(),
			traffic.task_messages_to_send.iter().map(|b| b.len()).max().unwrap()
		);

		//print len of messages of all tasks:
		println!("len of all messages of all tasks: {:?}", traffic.task_messages_to_send.iter().map(|b| b.len()).collect::<Vec<usize>>());
		traffic
	}

	fn get_all_meshblocks_list(&self) -> Vec<&Block> {
		let mut list = vec![];
		for root in &self.meshblocks_roots {
			list.extend(root.all_internal_meshblocks_list());
		}
		//list.sort_by_key(|b| b.id.unwrap()); //No need of ordering!
		list
	}

	fn refine_mesh_and_update_variables(&mut self) {
		for refinement_round in 0..self.refinement_pattern.len()
		{
			let param = ManyToManyParam {
				list: (0..self.total_meshblocks).collect(),
				origin: None,
				current: None,
				destination: None,
				extra: None,
			};
			let mut to_refine = self.refinement_pattern[refinement_round].get_destination(param, None, &mut self.rng);
			let mut id_start_label = 0;
			for i in 0..self.meshblocks_roots.len() {
				id_start_label = self.meshblocks_roots[i].refine_and_relabel_tree(&mut to_refine, self.max_levels, &self.meshblock_spaces_by_level, id_start_label);
			}
			self.total_meshblocks = id_start_label;
		}
	}

	fn assign_blocks_to_tasks(&mut self) {
		let blocks_per_task = self.total_meshblocks / self.tasks;
		let mut remaining = self.total_meshblocks % self.tasks;
		let mut assigned = 0;
		self.block_to_task = vec![0; self.total_meshblocks];
		for i in 0..self.tasks {
			for _ in 0..blocks_per_task {
				self.tasks_to_block[i].push(assigned);
				self.block_to_task[assigned] = i;
				assigned += 1;
			}
			if remaining != 0 {
				self.tasks_to_block[i].push(assigned);
				self.block_to_task[assigned] = i;
				assigned += 1;
				remaining -= 1;
			}
		}
		if assigned != self.total_meshblocks {
			panic!("Not all blocks were assigned to tasks")
		}
	}

	fn generate_amr_step_messages(&mut self) {
		// let all_meshblocks_list:Vec<(Vec<usize>, usize)> = self.get_all_meshblocks_list().iter().map(|b| (b.coordinates.clone(), b.level)).collect();
		let all_meshblocks_list = self.get_all_meshblocks_list();
		let mut task_messages_to_send = vec![VecDeque::new(); self.tasks];
		let cartesian_levels: Vec<CartesianData> = (0..self.max_levels).map(|level| CartesianData::new(&self.meshblock_spaces_by_level[level])).collect();
		for i in 0..self.tasks {
			for j in 0..self.tasks_to_block[i].len() {
				let block_id = self.tasks_to_block[i][j];
				let block_coords = all_meshblocks_list[block_id].coordinates.clone();
				let level = all_meshblocks_list[block_id].level.clone();
				let neighbours = self.neighbour_patterns[level].get_destination(cartesian_levels[level].pack(&*block_coords), None, &mut self.rng.clone());
				let mut final_neighbours = HashSet::new();

				for k in neighbours {
					let neighbour_coord = cartesian_levels[level].unpack(k);
					let root_block = neighbour_coord.clone().iter_mut().map(|x| *x / 2usize.pow(level as u32)).collect::<Vec<usize>>();
					let root_offset = self.meshblock_label.get_destination(cartesian_levels[0].pack(&root_block), None, &mut self.rng.clone()); //its ID??
					let root_children = self.meshblocks_roots[root_offset].all_internal_meshblocks_list();
					for c in root_children {
						if self.check_neighbour(&block_coords, &neighbour_coord, level, &c.coordinates, c.level) {
							final_neighbours.insert(c.id.unwrap());
						}
					}
				}
				// task_block_neighbours.extend(final_neighbours.into_iter());
				for nei in final_neighbours.iter() { //Add message to send!
					let message_size = self.message_size / (cmp::max(all_meshblocks_list[*nei].level, level) +1usize).pow(2);
					let task_to_send = self.block_to_task[*nei];
					if task_to_send == i{
						continue; //dont send messages to itself or it will crash!
					}
					task_messages_to_send[i].push_back((task_to_send, message_size));
				}
			}
		}
		self.task_messages_to_send = task_messages_to_send;
		self.messages_to_consume = self.task_messages_to_send.iter().map(|a| a.len()).reduce(|a, b| a + b).unwrap();
	}

	fn check_neighbour(&self, origin_meshblock: &Vec<usize>, neighbour_coord: &Vec<usize>, origin_neighbour_level: usize, destination_root_child: &Vec<usize>, destination_root_child_level: usize) -> bool {
		if origin_neighbour_level >= destination_root_child_level && neighbour_coord.iter().zip(destination_root_child.iter()).all(|(a, b)| *a == (b / 2usize.pow(origin_neighbour_level as u32 - destination_root_child_level as u32))) {
			true //If the obtained neighbour is more refined than what its in the mesh, and the coordinates coincides, TRUE
		} else if origin_neighbour_level < destination_root_child_level {
			let origin_meshblock_children = Block::get_ordered_children_coordinates_from_block_coord(&origin_meshblock);
			let new_children_aux_level = origin_neighbour_level + 1;
			let new_cartesian_dimension = CartesianData::new(&self.meshblock_spaces_by_level[new_children_aux_level]);
			for origin_child in origin_meshblock_children.iter() {
				let new_smaller_neighbours = self.neighbour_patterns[new_children_aux_level].get_destination(new_cartesian_dimension.pack(origin_child), None, &mut self.rng.clone());
				for origin_child_neighbours in new_smaller_neighbours {
					if self.check_neighbour(origin_child, &new_cartesian_dimension.unpack(origin_child_neighbours), new_children_aux_level, destination_root_child, destination_root_child_level) {
						return true
					}
				}
			}
			false
		} else {
			false
		}
	}
}

/**
Mini-Kernels which imitate the behavior of real applications.

## Wavefront
The wavefront traffic is applied over a n-dimensional space.
Starts at the corner of the space (0,0,...,0) and ends at the opposite corner (n-1, n-1,..., n-1).

```ignore
	Wavefront{
		task_space: [10,10,10],
		data_size: 16,
	}
```
## LinearAll2All
The LinearAll2All traffic generates all2all in each row, column, etc., depending on the task space.
It is like an FFT.

```ignore
        LinearAll2All{
                task_space: [10,10],
                message_size: 16,
        }
```
This could simulate a FFT3D, where each task contains a portion of data.

**/

#[derive(Quantifiable)]
#[derive(Debug)]
pub struct MiniApp {}

impl MiniApp {

	pub fn new(traffic: String, arg:TrafficBuilderArgument) -> Box<dyn Traffic> {

		let traffic_cv = match traffic.as_str() {

			"Wavefront" => {
				let mut task_space = None;
				let mut message_size = None;

				match_object_panic!(arg.cv, "Wavefront", value,
                    "task_space" => task_space = Some(value.as_array().expect("Bad task_space value").iter().map(|v| v.as_f64().expect("Bad task_space value") as usize).collect()),
                    "message_size" => message_size = Some(value.as_f64().expect("Bad data_size value") as usize),
                );

				let task_space = task_space.expect("task_space is required");
				let message_size = message_size.expect("message_size is required");

				get_wavefront(task_space, message_size)
			},

			"All2AllLinear" => {
				let mut task_space = None;
				let mut message_size = None;
				let mut rounds = None;

				match_object_panic!(arg.cv, "All2AllLinear", value,
					"task_space" => task_space = Some(value.as_array().expect("Bad task_space value").iter().map(|v| v.as_f64().expect("Bad task_space value") as usize).collect()),
					"message_size" => message_size = Some(value.as_f64().expect("Bad data_size value") as usize),
					"rounds" => rounds = Some(value.as_f64().expect("Bad rounds value") as usize),
				);

				let task_space = task_space.expect("task_space is required");
				let message_size = message_size.expect("message_size is required");
				let rounds = rounds.unwrap_or(1);

				get_all2all_linear(task_space, message_size, rounds)
			}

			_ => panic!("Unknown traffic type: {}", traffic),
		};
		new_traffic(TrafficBuilderArgument{cv: &traffic_cv, ..arg})
	}

}

fn get_wavefront(task_space: Vec<usize>, message_size:usize) -> ConfigurationValue{
	let mut neighbours_vector = vec![];
	for i in 0..task_space.len(){
		let mut neighbours = vec![0; task_space.len()];
		neighbours[i] = 1;
		neighbours_vector.push(neighbours);
	}
	let inmediate_neighbours_builder = ImmediateNeighboursCVBuilder {
		sides: task_space.clone(),
		vector_neighbours: neighbours_vector,
		modular: false,
	};

	let inmediate_neighbours = immediate_neighbours_cv_builder(inmediate_neighbours_builder);

	let total_tasks = task_space.iter().product();
	let message_to_vector_builder = SendMessageToVectorCVBuilder{
		tasks: total_tasks,
		one_to_many_pattern: inmediate_neighbours,
		message_size,
		rounds: 1,
	};
	let message_to_vector = build_send_message_to_vector_cv(message_to_vector_builder);
	// let start = ConfigurationValue::Object("Hotspots".to_string(), vec![("destinations".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(0.0)]))]);

	let identity_pattern_vector = vec![ConfigurationValue::Object("Identity".to_string(), vec![]); task_space.len()];
	let initial_credits = ConfigurationValue::Object("Sum".to_string(), vec![
		("patterns".to_string(), ConfigurationValue::Array(
			(0..task_space.len()).into_iter().enumerate().map(|(i, _z)| {
				let mut patterns_cartesian_transform = identity_pattern_vector.clone();
				patterns_cartesian_transform[i] = get_hotspot_destination(vec![0]); //ConfigurationValue::Object("Hotspots".to_string(), vec![("destinations".to_string(),ConfigurationValue::Array())]);
				let pattern_cad_sel = get_cartesian_transform(task_space.clone(), None, Some(patterns_cartesian_transform));
				get_candidates_selection(pattern_cad_sel, total_tasks)
			}).collect())
		),
		("middle_sizes".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Number(2f64); task_space.len()])),
	]);
	let traffic_manager_builder = BuildTrafficManagerCVArgs{
		tasks: total_tasks,
		credits_to_activate: task_space.len(),
		messages_per_transition: task_space.len(),
		credits_per_received_message: 1,
		traffic: message_to_vector,
		initial_credits,
	};

	let traffic_manager = get_traffic_manager(traffic_manager_builder);
	traffic_manager
}


fn get_all2all_linear(task_space: Vec<usize>, message_size: usize, rounds: usize) -> ConfigurationValue {
	let mut traffics_for_sequence = vec![];
	let total_tasks:usize = task_space.iter().product();

	for k in 0..task_space.len() {
		let mut all2all_dim = vec![];
		let mut specific_all2alls_side = task_space.clone();
		specific_all2alls_side.remove(k);
		let specific_all2alls = specific_all2alls_side.iter().product();
		let specific_all2alls_cartesian_data = CartesianData::new(&specific_all2alls_side);
		for i in 0..specific_all2alls {
			let all2all = super::collectives::get_all2all(task_space[k], message_size * task_space[k], rounds, None);
			let cartesian_data_graphs = specific_all2alls_cartesian_data.unpack(i);
			let mut shift_vector = cartesian_data_graphs.clone();
			shift_vector.insert(k, 0);
			let mut matrix = vec![ vec![0i32; 1]; task_space.len()];
			matrix[k][0] = 1;
			let linear_transform_args = BuildLinearTransformCV {
				source_size: vec![task_space[k]],
				target_size: task_space.clone(),
				matrix,
			};
			let linear_transform = get_linear_transform(linear_transform_args);
			let cartesian_transform_args = BuildCartesianTransformCV{
				sides: task_space.clone(),
				shift: Some(shift_vector),
				..Default::default()
			};
			let cartesian_transform = get_cartesian_transform_from_builder(cartesian_transform_args);

			let composition_pattern_args = BuildCompositionCV{
				patterns: vec![linear_transform, cartesian_transform],
				..Default::default()
			};

			let map = get_composition_pattern_cv(composition_pattern_args);

			let traffic_map_args = BuildTrafficMapCVArgs {
				tasks: total_tasks,
				application: all2all,
				map
			};
			let traffic_map = build_traffic_map_cv(traffic_map_args);
			all2all_dim.push(traffic_map);
		}

		let traffic_sum_args = BuildTrafficSumCVArgs {
			tasks: task_space.iter().product(),
			list: all2all_dim,
			..Default::default()
		};
		let t_sum = build_traffic_sum_cv(traffic_sum_args);
		traffics_for_sequence.push(t_sum);
	}

	ConfigurationValue::Object("Sequence".to_string(), vec![
		("traffics".to_string(), ConfigurationValue::Array(traffics_for_sequence)),
	])
}



#[cfg(test)]
mod tests {
	use std::vec;
	use super::*;

	#[test]
	fn test_wavefront() {
		let task_space = vec![10, 10, 10];
		let message_size = 16;
		let traffic = get_wavefront(task_space, message_size);
		println!("{}", traffic.format_terminal());
	}

	#[test]
	fn test_all2all_linear() {
		let task_space = vec![10, 10];
		let message_size = 16;
		let traffic = get_all2all_linear(task_space, message_size, 1);
		println!("{}", traffic.format_terminal());
	}
	use rand::prelude::StdRng;
	use rand::SeedableRng;
	use crate::config_parser::ConfigurationValue;
	use crate::general_pattern::pattern::extra::get_candidates_selection;
	use crate::Plugs;
	use crate::traffic::new_traffic;

	#[test]
	fn test_traffic_manager() {
		let switches = 4.0;
		let mut rng = StdRng::seed_from_u64(0);
		let one_to_many_pattern = ConfigurationValue::Object("AllNeighbours".to_string(), vec![]);
		let cv_builder = super::SendMessageToVectorCVBuilder {
			tasks: 4,
			one_to_many_pattern,
			message_size: 16,
			rounds: 2,
		};
		let cv = super::build_send_message_to_vector_cv(cv_builder);

		let initial_credits =  get_candidates_selection(ConfigurationValue::Object("Identity".to_string(), vec![]), switches as usize);
		let traffic_manager_builder = super::BuildTrafficManagerCVArgs{
			tasks: 4,
			credits_to_activate: 1,
			messages_per_transition: 1,
			credits_per_received_message: 1,
			traffic: cv,
			initial_credits,
		};
		let traffic_manager = super::get_traffic_manager(traffic_manager_builder);
		println!("{}",traffic_manager.format_terminal() );


		let traffic_builder = super::TrafficBuilderArgument{
			cv: &traffic_manager,
			rng: &mut rng,
			plugs: &Plugs::default(),
			topology: None,
		};

		//Starts the traffic
		let mut t = new_traffic(traffic_builder);
		assert_eq!(t.number_tasks(), switches as usize);
		for _ in 0..(switches as usize -1) // extract all messages
		{
			let mut messages = vec![];

			for i in 0..switches as usize {
				assert_eq!(t.should_generate(i, 0, &mut rng), true); //inserting all2all
			}

			for i in 0..switches as usize {
				let message = t.generate_message(i, 0, None, &mut rng).unwrap();
				messages.push(message);
			}

			//now check that tasks are done and their state is FinishedGenerating. also traffic is not finished.
			for i in 0..(switches as usize) {
				assert_eq!(t.should_generate(i, 0, &mut rng), false);
			}

			for i in 0..messages.len() {
				assert_eq!(t.consume(messages[i].destination, &*messages[i], 0, None, &mut rng), true);
			}
		}

		assert_eq!(t.is_finished(Some(&mut rng)), false);

		for _ in 0..(switches as usize -1) // extract all messages
		{
			let mut messages = vec![];

			for i in 0..switches as usize {
				assert_eq!(t.should_generate(i, 0, &mut rng), true); //inserting all2all
			}

			for i in 0..switches as usize {
				let message = t.generate_message(i, 0, None, &mut rng).unwrap();
				messages.push(message);
			}

			//now check that tasks are done and their state is FinishedGenerating. also traffic is not finished.
			for i in 0..(switches as usize) {
				assert_eq!(t.should_generate(i, 0, &mut rng), false);
			}

			for i in 0..messages.len() {
				assert_eq!(t.consume(messages[i].destination, &*messages[i], 0, None, &mut rng), true);
			}
		}

		assert_eq!(t.is_finished(Some(&mut rng)), true);
	}

	#[test]
	fn test_message_size_modifier() {
		let switches = 4.0;
		let mut rng = StdRng::seed_from_u64(0);
		let one_to_many_pattern = ConfigurationValue::Object("AllNeighbours".to_string(), vec![]);
		let cv_builder = super::SendMessageToVectorCVBuilder {
			tasks: 4,
			one_to_many_pattern,
			message_size: 16,
			rounds: 2,
		};
		let cv = super::build_send_message_to_vector_cv(cv_builder);

		let initial_credits = get_candidates_selection(ConfigurationValue::Object("Identity".to_string(), vec![]), switches as usize);
		let traffic_manager_builder = super::BuildTrafficManagerCVArgs {
			tasks: 4,
			credits_to_activate: 1,
			messages_per_transition: 1,
			credits_per_received_message: 1,
			traffic: cv,
			initial_credits,
		};
		let traffic_manager = super::get_traffic_manager(traffic_manager_builder);

		let message_sizes = vec![16, 32, 64, 128, 256, 512];
		let message_size_modifier = super::get_message_size_modifier(super::BuildMessageSizeModifierCVArgs {
			tasks: 4,
			traffic: traffic_manager,
			message_sizes: message_sizes.clone(),
		});
		println!("{}", message_size_modifier.format_terminal());

		let traffic_builder = super::TrafficBuilderArgument {
			cv: &message_size_modifier,
			rng: &mut rng,
			plugs: &Plugs::default(),
			topology: None,
		};

		//Starts the traffic
		let mut t = new_traffic(traffic_builder);
		assert_eq!(t.number_tasks(), switches as usize);
		for iteration in 0..(switches as usize -1) // extract all messages
		{
			let mut messages = vec![];

			for i in 0..switches as usize {
				assert_eq!(t.should_generate(i, 0, &mut rng), true); //inserting all2all
			}

			for i in 0..switches as usize {
				let message = t.generate_message(i, 0, None, &mut rng).unwrap();
				assert_eq!(message.size, message_sizes[iteration]);
				messages.push(message);
			}

			//now check that tasks are done and their state is FinishedGenerating. also traffic is not finished.
			for i in 0..(switches as usize) {
				assert_eq!(t.should_generate(i, 0, &mut rng), false);
			}

			for i in 0..messages.len() {
				assert_eq!(t.consume(messages[i].destination, &*messages[i], 0, None, &mut rng), true);
			}
		}

		assert_eq!(t.is_finished(Some(&mut rng)), false);

		for iteration in 0..(switches as usize -1) // extract all messages
		{
			let mut messages = vec![];

			for i in 0..switches as usize {
				assert_eq!(t.should_generate(i, 0, &mut rng), true); //inserting all2all
			}

			for i in 0..switches as usize {
				let message = t.generate_message(i, 0, None, &mut rng).unwrap();
				assert_eq!(message.size, message_sizes[iteration + (switches as usize -1)]);
				messages.push(message);
			}

			//now check that tasks are done and their state is FinishedGenerating. also traffic is not finished.
			for i in 0..(switches as usize) {
				assert_eq!(t.should_generate(i, 0, &mut rng), false);
			}

			for i in 0..messages.len() {
				assert_eq!(t.consume(messages[i].destination, &*messages[i], 0, None, &mut rng), true);
			}
		}

		assert_eq!(t.is_finished(Some(&mut rng)), true);
	}

	#[test]
	fn test_block_struct() {

		let mut root_block = Block {
			id: Some(0),
			coordinates: vec![0, 0, 0],
			children: vec![],
			level: 0,
		};

		// Test refine_block
		root_block.refine_block();
		assert_eq!(root_block.children.len(), 8);
		assert!(root_block.id.is_none());
		for i in 0..8 {
			assert_eq!(root_block.children[i].id, Some(i));
			assert_eq!(root_block.children[i].level, 1);
		}

		// Test all_internal_meshblocks_list
		let list = root_block.all_internal_meshblocks_list();
		assert_eq!(list.len(), 8);

		// Test refine_and_relabel_tree
		let mut to_refine = vec![0, 7];
		let max_levels = 3;
		let spaces_by_level = vec![vec![1,1,1], vec![2,2,2], vec![4,4,4]];
		let next_id = root_block.refine_and_relabel_tree(&mut to_refine, max_levels, &spaces_by_level, 0);

		assert_eq!(next_id, 22);

		let list = root_block.all_internal_meshblocks_list();
		assert_eq!(list.len(), 22);
		let mut ids: Vec<usize> = list.iter().map(|b| b.id.unwrap()).collect();
		ids.sort();
		let expected_ids: Vec<usize> = (0..22).collect();
		assert_eq!(ids, expected_ids);
	}

	#[test]
	fn test_amr() {
		let mut rng = StdRng::seed_from_u64(0);
		let plugs = Plugs::default();
		let tasks = 7;
		let meshblock_space = vec![2, 2];
		let max_levels = 2;
		let message_size = 256;

		let amr_cv = ConfigurationValue::Object("AMR".to_string(), vec![
			("tasks".to_string(), ConfigurationValue::Number(tasks as f64)),
			("meshblock_space".to_string(), ConfigurationValue::Array(meshblock_space.iter().map(|&x| ConfigurationValue::Number(x as f64)).collect())),
			("max_levels".to_string(), ConfigurationValue::Number(max_levels as f64)),
			("meshblock_label".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
			// ("sub_meshblock_label".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
			("neighbour_selection".to_string(), ConfigurationValue::Literal("KingNeighbours".to_string())),
			("refinement_pattern".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Object("MinFilter".to_string(), vec![])])),
			("message_size".to_string(), ConfigurationValue::Number(message_size as f64)),
		]);

		let traffic_builder = TrafficBuilderArgument {
			cv: &amr_cv,
			rng: &mut rng,
			plugs: &plugs,
			topology: None,
		};

		let mut amr = AMR::new(traffic_builder);

		// Test refine_mesh_and_update_variables
		assert_eq!(amr.total_meshblocks, 7);

		// Test assign_blocks_to_tasks
		assert_eq!(amr.block_to_task.len(), amr.total_meshblocks);
		assert_eq!(amr.tasks_to_block.len(), tasks);
		let total_assigned_blocks: usize = amr.tasks_to_block.iter().map(|v| v.len()).sum();
		assert_eq!(total_assigned_blocks, amr.total_meshblocks);
		for i in 0..7 {
			assert_eq!(amr.tasks_to_block[i].len(), 1);
		}


		// Test generate_amr_step_messages
		let total_messages: usize = amr.task_messages_to_send.iter().map(|d| d.len()).sum();
		assert!(total_messages > 0);
		assert_eq!(amr.messages_to_consume, total_messages);

		// Test check_neighbour
		let origin = vec![0,0];
		let neighbour_of_origin = vec![0,1];
		let dest_child = vec![0,1];
		assert!(amr.check_neighbour(&origin, &neighbour_of_origin,0, &dest_child, 0));

		let not_neighbour = vec![1,1];
		assert!(!amr.check_neighbour(&origin, &not_neighbour, 0, &dest_child, 0));
	}

	#[test]
	fn test_amr_3d_random() {
		let mut rng = StdRng::seed_from_u64(0);
		let plugs = Plugs::default();
		let tasks = 32;
		let meshblock_space = vec![4, 4, 4];
		let max_levels = 3;
		let message_size = 256;

		let amr_cv = ConfigurationValue::Object("AMR".to_string(), vec![
			("tasks".to_string(), ConfigurationValue::Number(tasks as f64)),
			("meshblock_space".to_string(), ConfigurationValue::Array(meshblock_space.iter().map(|&x| ConfigurationValue::Number(x as f64)).collect())),
			("max_levels".to_string(), ConfigurationValue::Number(max_levels as f64)),
			("meshblock_label".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
			// ("sub_meshblock_label".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
			("neighbour_selection".to_string(), ConfigurationValue::Literal("KingNeighbours".to_string())),
			("refinement_pattern".to_string(), ConfigurationValue::Array(vec![ ConfigurationValue::Object("RandomFilter".to_string(), vec![
				("elements_to_return".to_string(), ConfigurationValue::Number(2f64)),
				("source".to_string(), ConfigurationValue::False),
				("destination".to_string(), ConfigurationValue::False),
			])])),
			("message_size".to_string(), ConfigurationValue::Number(message_size as f64)),
		]);

		let traffic_builder = TrafficBuilderArgument {
			cv: &amr_cv,
			rng: &mut rng,
			plugs: &plugs,
			topology: None,
		};

		let mut amr = AMR::new(traffic_builder);

		// Just check that it initializes without panic, and check some basics
		assert!(amr.total_meshblocks >= 64); // it starts at 64, and should refine more

		// Test assign_blocks_to_tasks
		assert_eq!(amr.block_to_task.len(), amr.total_meshblocks);
		assert_eq!(amr.tasks_to_block.len(), tasks);
		let total_assigned_blocks: usize = amr.tasks_to_block.iter().map(|v| v.len()).sum();
		assert_eq!(total_assigned_blocks, amr.total_meshblocks);

		// Test generate_amr_step_messages
		let total_messages: usize = amr.task_messages_to_send.iter().map(|d| d.len()).sum();
		assert!(total_messages > 0);
		assert_eq!(amr.messages_to_consume, total_messages);
		//check the list of meshblocks and its IDs
		let list = amr.get_all_meshblocks_list();
		assert_eq!(list.len(), amr.total_meshblocks);
		let mut ids = list.iter().map(|b| b.id.unwrap()).collect::<Vec<usize>>();
		assert_eq!(ids, (0..amr.total_meshblocks).collect::<Vec<usize>>());
	}
	#[test]
	fn test_amr_3d_random_big() {
		let mut rng = StdRng::seed_from_u64(0);
		let plugs = Plugs::default();
		let tasks = 512;
		let meshblock_space = vec![16, 16, 16];
		let max_levels = 3;
		let message_size = 256;

		let amr_cv = ConfigurationValue::Object("AMR".to_string(), vec![
			("tasks".to_string(), ConfigurationValue::Number(tasks as f64)),
			("meshblock_space".to_string(), ConfigurationValue::Array(meshblock_space.iter().map(|&x| ConfigurationValue::Number(x as f64)).collect())),
			("max_levels".to_string(), ConfigurationValue::Number(max_levels as f64)),
			("meshblock_label".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
			// ("sub_meshblock_label".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
			("neighbour_selection".to_string(), ConfigurationValue::Literal("KingNeighbours".to_string())),
			("refinement_pattern".to_string(), ConfigurationValue::Array(vec![ConfigurationValue::Object("RandomFilter".to_string(), vec![
				("elements_to_return".to_string(), ConfigurationValue::Number(0f64)),
				("source".to_string(), ConfigurationValue::False),
				("destination".to_string(), ConfigurationValue::False),
			])])),
			("message_size".to_string(), ConfigurationValue::Number(message_size as f64)),
		]);

		let traffic_builder = TrafficBuilderArgument {
			cv: &amr_cv,
			rng: &mut rng,
			plugs: &plugs,
			topology: None,
		};

		let mut amr = AMR::new(traffic_builder);

		amr.should_generate(0, 0, &mut rng);


	}

	#[test]
	fn test_amr_z_filling() {
		let mut rng = StdRng::seed_from_u64(0);
		let plugs = Plugs::default();

		let amr_cv = ConfigurationValue::Object("AMR".to_string(), vec![
			("tasks".to_string(), ConfigurationValue::Number(64.0)),
			("meshblock_space".to_string(), ConfigurationValue::Array(vec![
				ConfigurationValue::Number(4.0),
				ConfigurationValue::Number(4.0),
				ConfigurationValue::Number(4.0),
			])),
			("max_levels".to_string(), ConfigurationValue::Number(2.0)),
			("meshblock_label".to_string(), ConfigurationValue::Object("LinearTransform".to_string(), vec![
				("source_size".to_string(), ConfigurationValue::Array(vec![
					ConfigurationValue::Number(2.0), ConfigurationValue::Number(2.0), ConfigurationValue::Number(2.0),
					ConfigurationValue::Number(2.0), ConfigurationValue::Number(2.0), ConfigurationValue::Number(2.0),
				])),
				("target_size".to_string(), ConfigurationValue::Array(vec![
					ConfigurationValue::Number(2.0), ConfigurationValue::Number(2.0), ConfigurationValue::Number(2.0),
					ConfigurationValue::Number(2.0), ConfigurationValue::Number(2.0), ConfigurationValue::Number(2.0),
				])),
				("matrix".to_string(), ConfigurationValue::Array(vec![
					ConfigurationValue::Array(vec![ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0)]),
					ConfigurationValue::Array(vec![ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0)]),
					ConfigurationValue::Array(vec![ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0)]),
					ConfigurationValue::Array(vec![ConfigurationValue::Number(0.0), ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0)]),
					ConfigurationValue::Array(vec![ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(1.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0)]),
					ConfigurationValue::Array(vec![ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(0.0), ConfigurationValue::Number(1.0)]),
				])),
				("legend_name".to_string(), ConfigurationValue::Literal("Z-filling".to_string())),
			])),
			// ("sub_meshblock_label".to_string(), ConfigurationValue::Object("Identity".to_string(), vec![])),
			("neighbour_selection".to_string(), ConfigurationValue::Literal("KingNeighbours".to_string())),
			("refinement_pattern".to_string(), ConfigurationValue::Array(vec![ ConfigurationValue::Object("RandomFilter".to_string(), vec![
				("source".to_string(), ConfigurationValue::False),
				("destination".to_string(), ConfigurationValue::False),
				("elements_to_return".to_string(), ConfigurationValue::Number(0.0)),
			])])),
			("message_size".to_string(), ConfigurationValue::Number(16.0)),
		]);

		let traffic_builder = TrafficBuilderArgument {
			cv: &amr_cv,
			rng: &mut rng,
			plugs: &plugs,
			topology: None,
		};

		let mut amr = AMR::new(traffic_builder);
		assert_eq!(amr.tasks, 64);
		assert_eq!(amr.total_meshblocks, 64);
	}
}
