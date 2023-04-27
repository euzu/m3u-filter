use std::cell::RefCell;
use enum_iterator::all;
use std::collections::{HashMap};
use pest::iterators::Pair;
use pest::Parser;
use petgraph::algo::toposort;
use crate::m3u::PlaylistItem;
use crate::model::ItemField;
use petgraph::graph::DiGraph;


pub fn get_field_value(pli: &PlaylistItem, field: &ItemField) -> String {
    let header = pli.header.borrow();
    let value = match field {
        ItemField::Group => header.group.as_str(),
        ItemField::Name => header.name.as_str(),
        ItemField::Title => header.title.as_str(),
        ItemField::Url => pli.url.as_str(),
    };
    String::from(value)
}

pub fn set_field_value(pli: &mut PlaylistItem, field: &ItemField, value: String) -> () {
    let header = &mut pli.header.borrow_mut();
    match field {
        ItemField::Group => header.group = value,
        ItemField::Name => header.name = value,
        ItemField::Title => header.title = value,
        ItemField::Url => {}
    };
}

pub struct ValueProvider<'a> {
    pub(crate) pli: RefCell<&'a PlaylistItem>,
}

impl<'a> ValueProvider<'a> {
    fn call(&self, field: & ItemField) -> String {
        let pli = *self.pli.borrow();
        get_field_value(pli, field)
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
            value: self.value.clone(),
        }
    }
}

#[derive(Debug)]
pub struct RegexWithCaptures {
    pub restr: String,
    pub re: regex::Regex,
    pub captures: Vec<String>,
}

impl Clone for RegexWithCaptures {
    fn clone(&self) -> Self {
        RegexWithCaptures {
            restr: self.restr.clone(),
            re: self.re.clone(),
            captures: self.captures.clone(),
        }
    }
}


#[derive(Parser)]
//#[grammar = "filter.pest"]
#[grammar_inline = "WHITESPACE = _{ \" \" | \"\\t\" }\nfield = { \"Group\" | \"Title\" | \"Name\" | \"Url\" }\nand = {\"AND\" | \"and\"}\nor = {\"OR\" | \"or\"}\nnot = { \"NOT\" | \"not\" }\nregexp = @{ \"\\\"\" ~ ( \"\\\\\\\"\" | (!\"\\\"\" ~ ANY) )* ~ \"\\\"\" }\ncomparison_value = _{ regexp }\ncomparison = { field ~ \"~\" ~ comparison_value }\nbool_op = { and | or}\nexpr_group = { \"(\" ~ expr ~ \")\" }\nexpr = {comparison ~ (bool_op ~ expr)* | expr_group ~ (bool_op ~ expr)* | not ~ expr ~ (bool_op ~ expr)* }\nstmt = { expr  ~ (bool_op ~ expr)* }\nmain = _{ SOI ~ stmt ~ EOI }"]

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

impl std::fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            BinaryOperator::OR => write!(f, "OR"),
            BinaryOperator::AND => write!(f, "AND"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Filter {
    Group(Box<Filter>),
    Comparison(ItemField, RegexWithCaptures),
    UnaryExpression(UnaryOperator, Box<Filter>),
    BinaryExpression(Box<Filter>, BinaryOperator, Box<Filter>),
}

impl Filter {
    pub fn filter(&self, provider: &ValueProvider, processor: &mut dyn ValueProcessor, verbose: bool) -> bool {
        match self {
            Filter::Comparison(field, rewc) => {
                let value = provider.call(&field);
                let is_match = rewc.re.is_match(value.as_str());
                if is_match {
                    if verbose { println!("Match found: {:?} {} => {}={}", &rewc, &rewc.restr, &field, &value) }
                    processor.process(field, &value, rewc, verbose);
                }
                is_match
            }
            Filter::Group(expr) => {
                expr.filter(provider, processor, verbose)
            }
            Filter::UnaryExpression(op, expr) => {
                match op {
                    UnaryOperator::NOT => !expr.filter(provider, processor, verbose),
                }
            }
            Filter::BinaryExpression(left, op, right) => {
                match op {
                    BinaryOperator::AND => left.filter(provider, processor, verbose)
                        && right.filter(provider, processor, verbose),
                    BinaryOperator::OR => left.filter(provider, processor, verbose)
                        || right.filter(provider, processor, verbose),
                }
            }
        }
    }
}

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Filter::Comparison(field, rewc) => {
                write!(f, "{} ~ \"{}\"", field, String::from(&rewc.restr))
            }
            Filter::Group(stmt) => {
                write!(f, "({})", stmt)
            }
            Filter::UnaryExpression(op, expr) => {
                let flt = match op {
                    UnaryOperator::NOT => format!("NOT {}", expr),
                };
                write!(f, "{}", flt)
            }
            Filter::BinaryExpression(left, op, right) => {
                write!(f, "{} {} {}", left, op, right)
            }
        }
    }
}

macro_rules! exit {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        std::process::exit(1);
    }};
}

fn get_parser_item_field(expr: Pair<Rule>) -> ItemField {
    match expr.as_rule() {
        Rule::field => {
            let field_text = expr.as_str();
            for item in all::<ItemField>() {
                if field_text.eq_ignore_ascii_case(item.to_string().as_str()) {
                    return item;
                }
            }
        }
        _ => {}
    }
    exit!("unknown field: {}", expr.as_str());
}

fn get_parser_regexp(expr: Pair<Rule>, templates: &Vec<PatternTemplate>, verbose: bool) -> RegexWithCaptures {
    match expr.as_rule() {
        Rule::regexp => {
            let mut parsed_text = String::from(expr.as_str());
            parsed_text.pop();
            parsed_text.remove(0);
            let mut regstr = String::from(parsed_text.as_str());
            for t in templates {
                regstr = regstr.replace(format!("!{}!", &t.name).as_str(), &t.value);
            }
            let re = regex::Regex::new(regstr.as_str());
            if re.is_err() {
                exit!("cant parse regex: {}", regstr);
            }
            let regexp = re.unwrap();
            let captures = regexp.capture_names()
                .filter_map(|x| x).map(|x| String::from(x)).filter(|x| x.len() > 0).collect::<Vec<String>>();
            if verbose { println!("Created regex: {} with captures: [{}]", regstr, (&captures).join(", ")) }
            return RegexWithCaptures {
                restr: regstr,
                re: regexp,
                captures,
            };
        }
        _ => {}
    }
    exit!("unknown field: {}", expr.as_str());
}

fn get_parser_comparison(expr: Pair<Rule>, templates: &Vec<PatternTemplate>, verbose: bool) -> Filter {
    let mut expr_inner = expr.into_inner();
    let field = get_parser_item_field(expr_inner.next().unwrap());
    let regexp = get_parser_regexp(expr_inner.next().unwrap(), templates, verbose);
    Filter::Comparison(field, regexp)
}

macro_rules! handle_expr {
    ($bop: expr, $uop: expr, $stmts: expr, $exp: expr) => {
        {
            let result = match $bop {
                Some(binop) => {
                    let lhs = $stmts.pop().unwrap();
                    $bop = None;
                    Filter::BinaryExpression(Box::new(lhs), binop.clone(), Box::new($exp))
                },
                _ => match $uop {
                    Some(unop) => {
                        $uop = None;
                        Filter::UnaryExpression(unop.clone(), Box::new($exp))
                    },
                    _ => $exp
                }
            };
            $stmts.push(result);
        }
    }
}

fn get_parser_expression(expr: Pair<Rule>, templates: &Vec<PatternTemplate>, verbose: bool) -> Filter {
    let mut stmts = Vec::new();
    let pairs = expr.into_inner();
    let mut bop: Option<BinaryOperator> = None;
    let mut uop: Option<UnaryOperator> = None;

    for pair in pairs {
        match pair.as_rule() {
            Rule::comparison => {
                handle_expr!(bop, uop, stmts, get_parser_comparison(pair, templates, verbose));
            }
            Rule::expr => {
                handle_expr!(bop, uop, stmts, get_parser_expression(pair, templates, verbose));
            }
            Rule::expr_group => {
                handle_expr!(bop, uop, stmts, Filter::Group(Box::new(get_parser_expression(pair.into_inner().next().unwrap(), templates, verbose))));
            }
            Rule::not => {
                uop = Some(UnaryOperator::NOT);
            }
            Rule::bool_op => {
                bop = Some(get_parser_binary_op(pair.into_inner().next().unwrap()));
            }
            _ => {
                println!("did not expect rule: {:?}", pair)
            }
        }
    }
    if stmts.len() < 1 || stmts.len() > 1 {
        exit!("did not expect multiple rule: {:?}", stmts);
    }
    stmts.pop().unwrap()
}

fn get_parser_binary_op(expr: Pair<Rule>) -> BinaryOperator {
    match expr.as_rule() {
        Rule::and => BinaryOperator::AND,
        Rule::or => BinaryOperator::OR,
        _ => {
            exit!("Unknown  binray operator {}", expr.as_str());
        }
    }
}

pub fn get_filter(filter_text: &str, templates: Option<&Vec<PatternTemplate>>, verbose: bool) -> Filter {
    let empty_list = Vec::new();
    let template_list: &Vec<PatternTemplate> = templates.unwrap_or(&empty_list);
    let mut source = String::from(filter_text);
    for t in template_list {
        source = source.replace(format!("!{}!", &t.name).as_str(), &t.value);
    }

    let pairs = FilterParser::parse(Rule::main, &source).unwrap_or_else(|e| panic!("{}", e));

    let mut result: Option<Filter> = None;
    let mut op: Option<BinaryOperator> = None;
    for pair in pairs {
        match pair.as_rule() {
            Rule::stmt => {
                for expr in pair.into_inner() {
                    match expr.as_rule() {
                        Rule::expr => {
                            let expr = get_parser_expression(expr, template_list, verbose);
                            match &op {
                                Some(binop) => {
                                    result = Some(Filter::BinaryExpression(Box::new(result.unwrap()), binop.clone(), Box::new(expr)));
                                    op = None;
                                }
                                _ => result = Some(expr)
                            }
                        }
                        Rule::bool_op => {
                            op = Some(get_parser_binary_op(expr.into_inner().next().unwrap()));
                        }
                        _ => {
                            println!("unknown expression {:?}", expr);
                            //exit(format!("unknown stmt inner: {}", expr.as_str()).as_str());
                        }
                    }
                }
            }
            Rule::EOI => {}
            _ => {
                exit!("unknown: {}", pair.as_str());
            }
        }
    }
    match result {
        Some(filter) => filter,
        _ => {
            exit!("Unable to parse filter: {}", &filter_text);
        }
    }
}

fn build_dependency_graph(templates: &Vec<PatternTemplate>) -> (DiGraph<String, ()>, HashMap<usize, String>, HashMap<&String, Vec<String>>, bool) {
    let regex = regex::Regex::new("!(.*?)!").unwrap();
    let mut graph = DiGraph::new();
    let mut node_ids = HashMap::new();
    let mut node_names = HashMap::new();
    let mut node_deps = HashMap::new();

    let mut add_node = |di_graph: &mut DiGraph<_, _>, node_name: &String| match node_ids.get(node_name) {
        Some(idx) => *idx,
        _ => {
            let key = node_name.clone();
            let idx = di_graph.add_node(node_name.clone());
            node_names.insert(idx.index(), key.clone());
            node_ids.insert(key, idx);
            idx
        }
    };

    for template in templates {
        let node_idx = add_node(&mut graph, &template.name);
        let edges = regex.captures_iter(&template.value)
            .filter(|caps| caps.len() > 1)
            .filter_map(|caps| caps.get(1))
            .map(|caps| String::from(caps.as_str()))
            .collect::<Vec<String>>();
        let mut iter = edges.iter();
        while let Some(edge) = iter.next() {
            let edge_idx = add_node(&mut graph, &edge);
            graph.add_edge(edge_idx, node_idx, ());
        }
        node_deps.insert(&template.name, edges);
    }
    let cycles: Vec<Vec<String>> = petgraph::algo::tarjan_scc(&graph)
        .into_iter()
        .filter(|scc| scc.len() > 1)
        .map(|scc| scc.iter().map(|&i| node_names.get(&i.index()).unwrap().clone()).collect())
        .collect();
    for cyclic in &cycles {
        println!("Cyclic template dependencies detected [{}]", cyclic.join(" <-> "))
    }

    (graph, node_names, node_deps, cycles.len() > 0)
}

pub fn prepare_templates(templates: &Vec<PatternTemplate>, verbose: bool) -> Vec<PatternTemplate> {
    let mut result: Vec<PatternTemplate> = templates.iter().map(|t| t.clone()).collect();
    let (graph, node_map, node_deps, cyclic) = build_dependency_graph(templates);
    if cyclic {
        exit!("Cyclic dependencies in templates detected!");
    } else {
        let mut dep_value_map: HashMap<&String, String> = templates.into_iter().map(|t| (&t.name, t.value.clone())).collect();
        // Perform a topological sort to get a linear ordering of the nodes
        let node_indices = toposort(&graph, None).unwrap();
        let mut indices = node_indices.iter();
        while let Some(node) = indices.next() {
            // only nodes with dependencies
            if graph.edges_directed(*node, petgraph::Incoming).count() > 0 {
                let node_name = node_map.get(&node.index()).unwrap();
                match node_deps.get(node_name) {
                    Some(deps) => {
                        if verbose { println!("template {}  depends on [{}]", node_name, deps.join(", ")) };
                        let mut node_template = dep_value_map.get(node_name).unwrap().clone();
                        for dep_name in deps {
                            let dep_template = dep_value_map.get(dep_name).unwrap().clone();
                            let new_templ = node_template.replace(format!("!{}!", dep_name).as_str(), &dep_template);
                            node_template = new_templ;
                        }
                        dep_value_map.insert(node_name, String::from(&node_template));
                        let template = result.iter_mut().find(|t| node_name.eq(&t.name)).unwrap();
                        //let new_value = dep_value_map.get(&template.name).unwrap();
                        template.value = String::from(&node_template);
                    }
                    _ => {}
                }
            }
        }
    }
    if verbose { println!("{:#?}", result); }
    result
}
