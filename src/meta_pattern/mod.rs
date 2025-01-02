/*!

A [GeneralPattern] defines the way elements select their destinations.

see [`new_pattern`](fn.new_pattern.html) for documentation on the configuration syntax of predefined patterns.

 */

pub mod pattern;
pub mod many_to_one_pattern;
pub mod one_to_many_pattern;
pub mod many_to_many_pattern;

use crate::meta_pattern::pattern::Pattern;
use rand::prelude::StdRng;
use crate::config_parser::ConfigurationValue;
use crate::meta_pattern::many_to_many_pattern::ManyToManyPattern;
use crate::Plugs;
use crate::meta_pattern::many_to_one_pattern::ManyToOnePattern;
use crate::meta_pattern::one_to_many_pattern::OneToManyPattern;
use crate::quantify::Quantifiable;
use crate::topology::Topology;

/// Some things most uses of the simple module will use.
pub mod prelude
{
    pub use super::{new_pattern, pattern::Pattern, MetaPatternBuilderArgument};
}

/// A Trait to create patterns (or functions).
/// The generic parameter E is the type of the argument passed to the pattern.
/// The generic parameter T is the type of the pattern's return value.
pub trait GeneralPattern<E, T>: Quantifiable + std::fmt::Debug{
    ///Initializes the pattern and variables.
    ///It performs any necessary setup/checks.
    ///Variables 'source_size' and 'destination_size', define the number of elements in the spaces where the pattern is going to be used.
    ///Topology (Optional) is the topology where the pattern is going to be used.
    fn initialize(&mut self, source_size:usize, target_size:usize, topology: Option<&dyn Topology>, rng: &mut StdRng);
    ///Returns the destination of an element.
    fn get_destination(&self, param: E, topology:Option<&dyn Topology>, rng: &mut StdRng)-> T;
}

///The argument to a builder function of multi_patterns.
#[derive(Debug)]
pub struct MetaPatternBuilderArgument<'a>
{
    ///A ConfigurationValue::Object defining the meta_pattern.
    pub cv: &'a ConfigurationValue,
    ///The user defined plugs. In case the meta_pattern needs to create elements.
    pub plugs: &'a Plugs,
}

impl<'a> MetaPatternBuilderArgument<'a>
{
    fn with_cv<'b>(&'b self, new_cv:&'b ConfigurationValue) -> MetaPatternBuilderArgument<'b>
    {
        MetaPatternBuilderArgument{
            cv: new_cv,
            plugs: self.plugs,
        }
    }
}

pub fn new_pattern(arg: MetaPatternBuilderArgument) -> Box<dyn Pattern>
{
   pattern::new_pattern(arg)
}

pub fn new_one_to_many_pattern(arg: MetaPatternBuilderArgument) -> Box<dyn OneToManyPattern>
{
    one_to_many_pattern::new_one_to_many_pattern(arg)
}

pub fn new_many_to_one_pattern(arg: MetaPatternBuilderArgument) -> Box<dyn ManyToOnePattern>
{
     many_to_one_pattern::new_many_to_one_pattern(arg)
}

pub fn new_many_to_many_pattern(arg: MetaPatternBuilderArgument) -> Box<dyn ManyToManyPattern>
{
    many_to_many_pattern::new_many_to_many_pattern(arg)
}