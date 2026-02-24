use std::rc::Rc;
use rand::{Rng, rngs::StdRng};
use crate::config_parser::ConfigurationValue;
use crate::{match_object_panic, Message};
use crate::topology::Topology;
use crate::event::Time;
use crate::traffic::{Traffic, TrafficBuilderArgument, TrafficError, TaskTrafficState};
use crate::general_pattern::{new_pattern, GeneralPatternBuilderArgument};
use crate::general_pattern::pattern::Pattern;
use quantifiable_derive::Quantifiable;
use std::f64::consts::PI;

/// Defines how the size of the messages is selected.
#[derive(Debug, Clone, Quantifiable)]
pub enum MessageSizeDistribution {
    Fixed(usize),
    Uniform(usize, usize), // min, max
    Normal { mean: f64, std_dev: f64 },
    Pareto { min: f64, shape: f64 }, // shape is alpha
    /// A finite set of messages with associated weights.
    /// Stored as (size, weight).
    Multimodal(Vec<usize>, Vec<f64>),
}

impl MessageSizeDistribution {
    pub fn new(cv: &ConfigurationValue) -> Self {
        match cv {
            ConfigurationValue::Number(n) => MessageSizeDistribution::Fixed(*n as usize),
            ConfigurationValue::Object(name, _pairs) => {
                match name.as_str() {
                    "Fixed" => {
                        let mut value = 0;
                        match_object_panic!(cv, "Fixed", v,
                            "value" => value = v.as_usize().expect("bad value for Fixed distribution"),
                        );
                        MessageSizeDistribution::Fixed(value)
                    },
                    "Uniform" => {
                        let mut min = 0;
                        let mut max = 0;
                        match_object_panic!(cv, "Uniform", v,
                            "min" => min = v.as_usize().expect("bad min for Uniform"),
                            "max" => max = v.as_usize().expect("bad max for Uniform"),
                        );
                        if min > max {
                            panic!("Uniform distribution min ({}) > max ({})", min, max);
                        }
                        MessageSizeDistribution::Uniform(min, max)
                    },
                    "Normal" => {
                        let mut mean = 0.0;
                        let mut std_dev = 1.0;
                        match_object_panic!(cv, "Normal", v,
                            "mean" => mean = v.as_f64().expect("bad mean for Normal"),
                            "std_dev" => std_dev = v.as_f64().expect("bad std_dev for Normal"),
                        );
                        MessageSizeDistribution::Normal { mean, std_dev }
                    },
                    "Pareto" => {
                        let mut min = 1.0;
                        let mut shape = 1.0;
                        match_object_panic!(cv, "Pareto", v,
                            "min" => min = v.as_f64().expect("bad min for Pareto"),
                            "shape" => shape = v.as_f64().expect("bad shape for Pareto"),
                        );
                        MessageSizeDistribution::Pareto { min, shape }
                    },
                    "Multimodal" => {
                        let mut sizes = vec![];
                        let mut weights = vec![];
                        match_object_panic!(cv, "Multimodal", v,
                            "sizes" => sizes = v.as_array().expect("bad sizes").iter().map(|x| x.as_usize().unwrap()).collect(),
                            "weights" => weights = v.as_array().expect("bad weights").iter().map(|x| x.as_f64().unwrap()).collect(),
                        );
                        if sizes.len() != weights.len() || sizes.is_empty() {
                            panic!("Multimodal sizes and weights must have same non-zero length");
                        }
                        MessageSizeDistribution::Multimodal(sizes, weights)
                    },
                    _ => panic!("Unknown distribution {}", name),
                }
            },
            _ => panic!("Distribution must be a Number or an Object"),
        }
    }

    pub fn sample(&self, rng: &mut StdRng) -> usize {
        let val = match self {
            MessageSizeDistribution::Fixed(v) => *v as f64,
            MessageSizeDistribution::Uniform(min, max) => rng.gen_range(*min..=*max) as f64,
            MessageSizeDistribution::Normal { mean, std_dev } => {
                let u1: f64 = rng.gen();
                let u2: f64 = rng.gen();
                let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();
                mean + z0 * std_dev
            },
            MessageSizeDistribution::Pareto { min, shape } => {
                let u: f64 = rng.gen();
                min / u.powf(1.0 / shape)
            },
            MessageSizeDistribution::Multimodal(sizes, weights) => {
                let total_weight: f64 = weights.iter().sum();
                let mut r = rng.gen::<f64>() * total_weight;
                for (i, &weight) in weights.iter().enumerate() {
                    r -= weight;
                    if r <= 0.0 {
                        return sizes[i];
                    }
                }
                *sizes.last().unwrap() as f64
            }
        };
        
        if val < 1.0 { 1 } else { val as usize }
    }

    pub fn average(&self) -> f64 {
        match self {
            MessageSizeDistribution::Fixed(v) => *v as f64,
            MessageSizeDistribution::Uniform(min, max) => (min + max) as f64 / 2.0,
            MessageSizeDistribution::Normal { mean, .. } => *mean,
            MessageSizeDistribution::Pareto { min, shape } => {
                if *shape <= 1.0 { f64::INFINITY } else { (shape * min) / (shape - 1.0) }
            },
            MessageSizeDistribution::Multimodal(sizes, weights) => {
                let total_weight: f64 = weights.iter().sum();
                let weighted_sum: f64 = sizes.iter().zip(weights.iter()).map(|(s, w)| *s as f64 * w).sum();
                weighted_sum / total_weight
            }
        }
    }
}

/// A generic traffic generator that combines a spatial Pattern with a Message Size Distribution.
/// Useful for simulating Datacenter/HPC workloads synthetically.
///
/// # Configuration
///
/// * `tasks`: Number of tasks (nodes) generating traffic.
/// * `load`: Offered load in flits/cycle/node.
/// * `message_size`: Distribution of message sizes.
/// * `pattern`: Spatial pattern of destinations.
///
/// # Examples
///
/// ## Datacenter Workload
///
/// Simulates a scenario with "Mice and Elephant" flows (bimodal size distribution) and
/// a Hotspot pattern where a few nodes receive most of the traffic.
///
/// ```ignore
/// SyntheticTrafficDistribution {
///     tasks: 1000,
///     // Sweep load to find saturation point (e.g., 0.1 to 0.9)
///     load: 0.6,
///     
///     // Hotspot Pattern: 50% of traffic goes to nodes 0-3, rest is uniform.
///     pattern: RandomMix {
///         patterns: [ Hotspots { destinations: [0, 1, 2, 3] }, Uniform ],
///         weights: [50, 50]
///     },
///
///     // Bimodal Size Distribution:
///     // 80% small control messages (1 phit), 20% large data packets (16 phits)
///     message_size: Multimodal {
///         sizes:   [1,   16],
///         weights: [0.8, 0.2]
///     }
/// }
/// ```
///
/// # Metrics to Analyze
///
/// When analyzing results from these synthetic traffics, consider:
///
/// * **Accepted Load (Throughput):** Compare `result.accepted_load` vs `configuration.traffic.load`.
/// * **Tail Latency (p99):** Look at `result.message_latency_p99`. In Datacenter workloads (bimodal),
///   small messages stuck behind large ones cause spikes in tail latency even if average latency is low.
/// * **Fairness:** In Hotspot or congested scenarios, check if some nodes are starved.

#[derive(Debug, Quantifiable)]
pub struct SyntheticTrafficDistribution {
    tasks: usize,
    load: f32,
    message_size: MessageSizeDistribution,
    pattern: Box<dyn Pattern>,
    generation_probability: f32,
}

impl SyntheticTrafficDistribution {
    pub fn new(arg: TrafficBuilderArgument) -> SyntheticTrafficDistribution {
        let mut tasks = None;
        let mut load = None;
        let mut message_size = None;
        let mut pattern = None;
        
        match_object_panic!(arg.cv, "SyntheticTrafficDistribution", value,
            "tasks" => tasks = Some(value.as_usize().expect("bad value for tasks")),
            "load" => load = Some(value.as_f64().expect("bad value for load") as f32),
            "message_size" => message_size = Some(MessageSizeDistribution::new(value)),
            "pattern" => pattern = Some(new_pattern(GeneralPatternBuilderArgument{cv:value, plugs:arg.plugs})),
        );

        let message_size = message_size.expect("message_size is required");
        let load = load.expect("load is required");
        let avg_size = message_size.average();
        let generation_probability = if avg_size > 0.0 { load / avg_size as f32 } else { 0.0 };

        let tasks = tasks.expect("tasks is required");
        let mut pattern = pattern.expect("pattern is required");
        pattern.initialize(tasks, tasks, arg.topology, arg.rng);

        SyntheticTrafficDistribution {
            tasks,
            load,
            message_size,
            pattern,
            generation_probability,
        }
    }
}

impl Traffic for SyntheticTrafficDistribution {
    fn generate_message(&mut self, origin: usize, cycle: Time, topology: Option<&dyn Topology>, rng: &mut StdRng) -> Result<Rc<Message>, TrafficError> {
        let destination = self.pattern.get_destination(origin, topology, rng);
        if destination == origin { return Err(TrafficError::SelfMessage); }
        if destination >= self.tasks { return Err(TrafficError::OriginOutsideTraffic); }
        Ok(Rc::new(Message { origin, destination, size: self.message_size.sample(rng), creation_cycle: cycle, payload: vec![], id_traffic: None }))
    }
    fn should_generate(&mut self, _task: usize, _cycle: Time, rng: &mut StdRng) -> bool {
        rng.gen::<f32>() < self.generation_probability
    }
    fn probability_per_cycle(&self, _task: usize) -> f32 { self.generation_probability }
    fn consume(&mut self, _task: usize, _message: &dyn crate::AsMessage, _cycle: Time, _topology: Option<&dyn Topology>, _rng: &mut StdRng) -> bool { true }
    fn is_finished(&mut self, _rng: Option<&mut StdRng>) -> bool { false }
    fn task_state(&mut self, _task: usize, _cycle: Time) -> Option<TaskTrafficState> { Some(TaskTrafficState::Generating) }
    fn number_tasks(&self) -> usize { self.tasks }
}