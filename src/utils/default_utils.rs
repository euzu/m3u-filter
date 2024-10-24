use std::collections::HashMap;
use std::rc::Rc;
use crate::model::config::ProcessingOrder;

pub(crate) fn default_as_true() -> bool { true }

pub(crate) fn default_as_false() -> bool { false }

pub(crate) fn default_as_empty_str() -> String { String::new() }

pub(crate) fn default_as_empty_rc_str() -> Rc<String> { Rc::new(String::new()) }

pub(crate) fn default_as_zero_u8() -> u8 { 0 }

pub(crate) fn default_as_frm() -> ProcessingOrder { ProcessingOrder::Frm }

pub(crate) fn default_as_default() -> String { String::from("default") }

pub(crate) fn default_as_empty_map<K, V>() -> HashMap<K, V> { HashMap::new() }

pub(crate) fn default_as_empty_list<T>() -> Vec<T> { vec![] }

pub(crate) fn default_as_two_u16() -> u16 { 2 }

pub(crate) fn default_as_zero_u32() -> u32 { 0 }
pub(crate) fn default_as_zero_u16() -> u16 { 0 }

