use enum_iterator::all;
use std::borrow::{Borrow};
use std::sync::{Arc, Mutex};
use pest::Parser;
use crate::m3u::PlaylistItem;
use crate::model::ItemField;

pub struct ValueProvider<'a> {
    pub(crate) pli: &'a PlaylistItem,
}

impl<'a> ValueProvider<'a> {
    fn call(&self, field: &'a ItemField) -> &str {
        return match field {
            ItemField::Group => &*self.pli.header.group.as_str(),
            ItemField::Name => &*self.pli.header.name.as_str(),
            ItemField::Title => &*self.pli.header.title.as_str(),
            ItemField::Url => &*self.pli.url.as_str(),
        }
    }
}

pub trait ValueProcessor {
    fn process(&mut self, field: &ItemField, value: &str, rewc: &RegexWithCaptures, verbose: bool) -> bool;
}

pub struct MockValueProcessor {}
impl ValueProcessor for MockValueProcessor {
    fn process(&mut self, _: &ItemField, _: &str, _: &RegexWithCaptures, _: bool) -> bool {
        return false;
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PatternTemplate {
    pub name: String,
    pub value: String,
}

impl Clone for PatternTemplate {
    fn clone(&self) -> Self {
        PatternTemplate {
            name: self.name.clone(),
            value: self.value.clone()
        }
    }
}

#[derive(Debug)]
pub struct RegexWithCaptures {
    pub re: regex::Regex,
    pub captures: Vec<String>,
}

impl Clone for RegexWithCaptures {
    fn clone(&self) -> Self {
        RegexWithCaptures {
            re: self.re.clone(),
            captures: self.captures.clone()
        }
    }
}


#[derive(Parser)]
#[grammar_inline = "field = { \"Group\" | \"Title\" | \"Name\" | \"Url\"}\nand = {\"AND\" | \"and\"}\nor = {\"OR\" | \"or\"}\nnot = { \"NOT\" | \"not\" }\nlparen = { \"(\" }\nrparen = { \")\" }\nregexp_op = _{ \"~\" }\nregexp = @{ \"\\\"\" ~ ( \"\\\\\\\"\" | (!\"\\\"\" ~ ANY) )* ~ \"\\\"\" }\nvalue = _{ regexp }\n\noperator = _{ regexp_op }\nmatch_comparison = _{ field ~ operator ~ value }\npredicate = _{ match_comparison }\nbool_test = _{ predicate | lparen ~ condition ~ rparen }\nbool_factor = _{ not? ~ bool_test }\nbool_term = _{ bool_factor ~ (and ~ bool_factor)* }\ncondition = _{ bool_term ~ (or ~ bool_term)* }\nmain = _{ SOI ~ condition ~ EOI }\nWHITESPACE = _{ \" \" | \"\\t\" }"]
struct FilterParser;

#[derive(Debug, Clone)]
pub enum UnaryOperator {
    NOT
}

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    AND,
    OR,
}

#[derive(Debug, Clone)]
pub enum Filter {
    Group(Arc<Mutex<Vec<Arc<Filter>>>>),
    Comparison(ItemField, Arc<Mutex<Option<RegexWithCaptures>>>),
    UnaryExpression(UnaryOperator, Arc<Mutex<Option<Arc<Filter>>>>),
    BinaryExpression(BinaryOperator, Arc<Filter>, Arc<Mutex<Option<Arc<Filter>>>>),
}

impl Filter {
    pub fn filter(&self, provider: &ValueProvider, processor: &mut dyn ValueProcessor, verbose: bool) -> bool {
        match self {
            Filter::Comparison(field, regex) => {
                return match &*regex.lock().unwrap() {
                    Some(rewc) => {
                        let value = provider.call(&field);
                        let is_match = rewc.re.is_match(value);
                        if is_match {
                            processor.process(field, &value, rewc, verbose);
                        }
                        return is_match
                    },
                    _ => false
                };
            },
            Filter::Group(stmts) => {
                for stmt in &*stmts.lock().unwrap() {
                    if !stmt.filter(provider, processor, verbose) {
                        return false;
                    }
                }
                return true;
            },
            Filter::UnaryExpression(op, expr) => {
                match op {
                    UnaryOperator::NOT => {
                        match &*expr.lock().unwrap() {
                            Some(e) => !e.filter(provider, processor, verbose),
                            _ => false
                        }
                    }
                }
            },
            Filter::BinaryExpression(op, left, right) => {
                return match &*right.lock().unwrap() {
                    Some(r) => return match op {
                        BinaryOperator::AND => left.filter(provider, processor, verbose)
                            && r.filter(provider, processor, verbose),
                        BinaryOperator::OR => left.filter(provider, processor, verbose)
                            || r.filter(provider, processor, verbose),
                    },
                    _ => false
                }
            }
        }
    }
}

fn exit(msg: &str) {
    println!("{}", msg);
    std::process::exit(1);
}

fn merge_with_stack(stack: &mut Vec<Arc<Filter>>, expr: &Arc<Filter>) -> bool {
    let item = stack.last();
    match item {
        Some(rcflt) => {
            let flt = rcflt.as_ref();
            match flt {
                Filter::Group(stmts) => {
                    (*stmts.lock().unwrap()).push(Arc::clone(expr));
                }
                Filter::UnaryExpression(_, value) => {
                    *value.lock().unwrap() = Some(Arc::clone(expr));
                    return true;
                }
                Filter::BinaryExpression(_, _, right) => {
                    *right.lock().unwrap() = Some(Arc::clone(expr));
                    return true;
                }
                _ => {}
            }
        }
        None => {}
    }
    return false;
}

fn compact_stack(stack: &mut Vec<Arc<Filter>>) {
    if stack.len() > 1 {
        match stack.last() {
            Some(rcflt) => {
                let flt = rcflt.as_ref();
                match flt {
                    Filter::Comparison(_, value) => {
                        if value.lock().unwrap().is_some() {
                            let e = &stack.pop().unwrap();
                            merge_with_stack(stack, e);
                            compact_stack(stack);
                        }
                    }
                    Filter::BinaryExpression(_, _, value) => {
                        if value.lock().unwrap().is_some() {
                            let e = &stack.pop().unwrap();
                            merge_with_stack(stack, e);
                            compact_stack(stack);
                        }
                    }
                    Filter::UnaryExpression(_, value) => {
                        if value.lock().unwrap().is_some() {
                            let e = &stack.pop().unwrap();
                            merge_with_stack(stack, e);
                            compact_stack(stack);
                        }
                    }
                    _ => {}
                }
            }
            None => {}
        }
    }
}

pub fn get_filter(source: &str, templates: Option<&Vec<PatternTemplate>>) -> Filter {
    let mut stack: Vec<Arc<Filter>> = vec![];
    let empty_vec = Vec::new();
    let templ : &Vec<PatternTemplate> = templates.unwrap_or(&empty_vec);

    let pairs = FilterParser::parse(Rule::main, source).unwrap_or_else(|e| panic!("{}", e));
    for pair in pairs {
        match pair.as_rule() {
            Rule::lparen => {
                let expr = Filter::Group(Arc::new(Mutex::new(Vec::<Arc<Filter>>::new())));
                compact_stack(&mut stack);
                stack.push(Arc::new(expr));
            }
            Rule::rparen => {
                if stack.len() > 1 {
                    stack.pop();
                }
            }
            Rule::not => {
                compact_stack(&mut stack);
                let expr = Filter::UnaryExpression(UnaryOperator::NOT, Arc::new(Mutex::new(None)));
                stack.push(Arc::new(expr));
            }
            Rule::and => {
                let left = stack.pop().unwrap();
                compact_stack(&mut stack);
                let expr = Filter::BinaryExpression(BinaryOperator::AND, Arc::clone(&left), Arc::new(Mutex::new(None)));
                stack.push(Arc::new(expr));
            }
            Rule::or => {
                let left = stack.pop().unwrap();
                compact_stack(&mut stack);
                let expr = Filter::BinaryExpression(BinaryOperator::OR, Arc::clone(&left), Arc::new(Mutex::new(None)));
                stack.push(Arc::new(expr));
            }
            Rule::field => {
                compact_stack(&mut stack);

                let mut field: Option<ItemField> = None;
                let field_text = pair.as_str();
                for item in all::<ItemField>() {
                    if field_text.eq_ignore_ascii_case(item.to_string().as_str()) {
                        field = Some(item);
                        break;
                    }
                }
                if field.is_none() {
                    exit((format!("unknown field: {}", field_text)).as_str());
                }

                let expr = Filter::Comparison(field.unwrap(), Arc::new(Mutex::new(None)));
                stack.push(Arc::new(expr));
            }
            Rule::regexp => {
                let mut parsed_text = String::from(pair.as_str());
                parsed_text.pop();
                parsed_text.remove(0);
                let mut regstr = String::from(parsed_text.as_str());
                for t in templ {
                    regstr = regstr.replace(format!("!{}!", &t.name).as_str(), &t.value);
                }
                let re = regex::Regex::new(regstr.as_str());
                if re.is_err() {
                    exit(format!("cant parse regex: {}", regstr).as_str());
                }
                let regexp = re.unwrap();
                let captures = regexp.capture_names().map(|x| String::from(x.unwrap_or(""))).filter(|x| x.len() > 0).collect::<Vec<String>>();
                let  regexp_with_captures = RegexWithCaptures {
                    re: regexp,
                    captures
                };
                let left = stack.last().unwrap();
                match left.borrow() {
                    Filter::Comparison(_, regex) => {
                            *regex.lock().unwrap() = Some(regexp_with_captures);
                        compact_stack(&mut stack);
                    }
                    _ => {}
                }
            }
            Rule::EOI => {}
            _ => {
                exit(format!("unknown: {}", pair.as_str()).as_str());
            }
        }
    }
    return Filter::Group(Arc::new(Mutex::new(stack)));
}
