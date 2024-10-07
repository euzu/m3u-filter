#![allow(clippy::empty_docs)]

use std::cell::RefCell;
use enum_iterator::all;
use std::collections::{HashMap};
use std::rc::Rc;
use log::{debug, error, Level, log_enabled};
use pest::iterators::Pair;
use pest::Parser;
use petgraph::algo::toposort;
use crate::model::playlist::{PlaylistItem, PlaylistItemType};
use crate::model::config::ItemField;
use petgraph::graph::DiGraph;
use crate::{create_m3u_filter_error_result, exit};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};


pub(crate) fn get_field_value(pli: &PlaylistItem, field: &ItemField) -> Rc<String> {
    let header = pli.header.borrow();
    let value = match field {
        ItemField::Group => &header.group,
        ItemField::Name => &header.name,
        ItemField::Title => &header.title,
        ItemField::Url => &header.url,
        ItemField::Type => &Rc::new(header.item_type.to_string()),
    };
    Rc::clone(value)
}

pub(crate) fn set_field_value(pli: &mut PlaylistItem, field: &ItemField, value: Rc<String>) {
    let header = &mut pli.header.borrow_mut();
    match field {
        ItemField::Group => header.group = value,
        ItemField::Name => header.name = value,
        ItemField::Title => header.title = value,
        ItemField::Url => header.url = value,
        ItemField::Type => {}
    };
}

pub(crate) struct ValueProvider<'a> {
    pub(crate) pli: RefCell<&'a PlaylistItem>,
}

impl<'a> ValueProvider<'a> {
    fn call(&self, field: &ItemField) -> Rc<String> {
        let pli = *self.pli.borrow();
        get_field_value(pli, field)
    }
}

pub(crate) trait ValueProcessor {
    fn process(&mut self, field: &ItemField, value: &str, rewc: &RegexWithCaptures) -> bool;
}

pub(crate) struct MockValueProcessor {}

impl ValueProcessor for MockValueProcessor {
    fn process(&mut self, _: &ItemField, _: &str, _: &RegexWithCaptures) -> bool {
        false
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PatternTemplate {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RegexWithCaptures {
    pub restr: String,
    pub re: regex::Regex,
    pub captures: Vec<String>,
}

#[derive(Parser)]
#[grammar_inline = r#"
WHITESPACE = _{ " " | "\t" }
field = { ^"group" | ^"title" | ^"name" | ^"url" }
and = { ^"and" }
or = { ^"or" }
not = { ^"not" }
regexp = @{ "\"" ~ ( "\\\"" | (!"\"" ~ ANY) )* ~ "\"" }
type_value = { ^"live" | ^"vod" | ^"series" }
type_comparison = { ^"type" ~ "=" ~ type_value }
field_comparison_value = _{ regexp }
field_comparison = { field ~ "~" ~ field_comparison_value }
comparison = { field_comparison | type_comparison }
bool_op = { and | or }
expr_group = { "(" ~ expr ~ ")" }
basic_expr = _{ comparison | expr_group }
not_expr = _{ not ~ basic_expr }
expr = {
  not_expr ~ (bool_op ~ expr)?
  | basic_expr ~ (bool_op ~ expr)*
}
stmt = { expr ~ (bool_op ~ expr)* }
main = _{ SOI ~ stmt ~ EOI }
"#]
struct FilterParser;

#[derive(Debug, Clone)]
pub(crate) enum UnaryOperator {
    Not
}

#[derive(Debug, Clone)]
pub(crate) enum BinaryOperator {
    And,
    Or,
}

impl std::fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            BinaryOperator::Or => write!(f, "OR"),
            BinaryOperator::And => write!(f, "AND"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Filter {
    Group(Box<Filter>),
    FieldComparison(ItemField, RegexWithCaptures),
    TypeComparison(ItemField, PlaylistItemType),
    UnaryExpression(UnaryOperator, Box<Filter>),
    BinaryExpression(Box<Filter>, BinaryOperator, Box<Filter>),
}

impl Filter {
    pub fn filter(&self, provider: &ValueProvider, processor: &mut dyn ValueProcessor) -> bool {
        match self {
            Filter::FieldComparison(field, rewc) => {
                let value = provider.call(field);
                let is_match = rewc.re.is_match(value.as_str());
                if is_match {
                    if log_enabled!(Level::Debug) {
                        debug!("Match found: {:?} {} => {}={}", &rewc, &rewc.restr, &field, &value);
                    }
                    processor.process(field, &value, rewc);
                }
                is_match
            }
            Filter::TypeComparison(field, item_type) => {
                let value = provider.call(field);
                match get_filter_item_type(value.as_str()) {
                    None => false,
                    Some(pli_type) => {
                        let is_match = pli_type.eq(item_type);
                        if is_match && log_enabled!(Level::Debug) {
                            debug!("Match found: {:?} {}", &field, value);
                        }
                        is_match
                    }
                }
            }
            Filter::Group(expr) => {
                expr.filter(provider, processor)
            }
            Filter::UnaryExpression(op, expr) => {
                match op {
                    UnaryOperator::Not => !expr.filter(provider, processor),
                }
            }
            Filter::BinaryExpression(left, op, right) => {
                match op {
                    BinaryOperator::And => left.filter(provider, processor)
                        && right.filter(provider, processor),
                    BinaryOperator::Or => left.filter(provider, processor)
                        || right.filter(provider, processor),
                }
            }
        }
    }
}

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Filter::FieldComparison(field, rewc) => {
                write!(f, "{} ~ \"{}\"", field, String::from(&rewc.restr))
            }
            Filter::TypeComparison(field, item_type) => {
                write!(f, "{} = {}", field, match item_type {
                    PlaylistItemType::Live => "live",
                    PlaylistItemType::Movie => "vod",
                    PlaylistItemType::Series => "series",
                    PlaylistItemType::SeriesInfo => "series" // yes series-info is handled as series in filter
                })
            }
            Filter::Group(stmt) => {
                write!(f, "({stmt})")
            }
            Filter::UnaryExpression(op, expr) => {
                let flt = match op {
                    UnaryOperator::Not => format!("NOT {expr}"),
                };
                write!(f, "{flt}")
            }
            Filter::BinaryExpression(left, op, right) => {
                write!(f, "{left} {op} {right}")
            }
        }
    }
}

fn get_parser_item_field(expr: &Pair<Rule>) -> Result<ItemField, M3uFilterError> {
    if expr.as_rule() == Rule::field {
        let field_text = expr.as_str();
        for item in all::<ItemField>() {
            if field_text.eq_ignore_ascii_case(item.to_string().as_str()) {
                return Ok(item);
            }
        }
    }
    create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "unknown field: {}", expr.as_str())
}

fn get_parser_regexp(expr: &Pair<Rule>, templates: &Vec<PatternTemplate>) -> Result<RegexWithCaptures, M3uFilterError> {
    if expr.as_rule() == Rule::regexp {
        let mut parsed_text = String::from(expr.as_str());
        parsed_text.pop();
        parsed_text.remove(0);
        let regstr  = apply_templates_to_pattern(&parsed_text, templates);
        let re = regex::Regex::new(regstr.as_str());
        if re.is_err() {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {}", regstr);
        }
        let regexp = re.unwrap();
        let captures = regexp.capture_names()
            .flatten().map(String::from).filter(|x| !x.is_empty()).collect::<Vec<String>>();
        if log_enabled!(Level::Debug) {
            debug!("Created regex: {} with captures: [{}]", regstr, captures.join(", "));
        }
        return Ok(RegexWithCaptures {
            restr: regstr,
            re: regexp,
            captures,
        });
    }
    create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "unknown field: {}", expr.as_str())
}

fn get_parser_field_comparison(expr: Pair<Rule>, templates: &Vec<PatternTemplate>) -> Result<Filter, M3uFilterError> {
    let mut expr_inner = expr.into_inner();
    match get_parser_item_field(&expr_inner.next().unwrap()) {
        Ok(field) => {
            match get_parser_regexp(&expr_inner.next().unwrap(), templates) {
                Ok(regexp) => Ok(Filter::FieldComparison(field, regexp)),
                Err(err) => Err(err),
            }
        }
        Err(err) => Err(err)
    }
}

fn get_filter_item_type(text_item_type: &str) -> Option<PlaylistItemType> {
    if text_item_type.eq_ignore_ascii_case("live") {
        Some(PlaylistItemType::Live)
    } else if text_item_type.eq_ignore_ascii_case("movie")  || text_item_type.eq_ignore_ascii_case("vod") {
        Some(PlaylistItemType::Movie)
    } else if text_item_type.eq_ignore_ascii_case("series") {
        Some(PlaylistItemType::Series)
    } else if text_item_type.eq_ignore_ascii_case("series-info") {
        // this is necessarry to avoid series and series-info confusion in filter!
        // we can now use series  for filtering series and series-info (series-info are categories)
        Some(PlaylistItemType::Series)
    } else {
        None
    }
}

fn get_parser_type_comparison(expr: Pair<Rule>) -> Result<Filter, M3uFilterError> {
    let expr_inner = expr.into_inner();
    let text_item_type = expr_inner.as_str();
    let item_type = get_filter_item_type(text_item_type);
    match item_type {
        None => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse item type: {text_item_type}"),
        Some(itype) => Ok(Filter::TypeComparison(ItemField::Type, itype))
    }
}

macro_rules! handle_expr {
    ($bop: expr, $uop: expr, $stmts: expr, $exp: expr) => {
        {
            let result = match $bop {
                Some(binop) => {
                    let lhs = $stmts.pop().unwrap();
                    $bop = None;
                    Filter::BinaryExpression(Box::new(lhs), binop.clone(), Box::new($exp))
                }
                _ => match $uop {
                    Some(unop) => {
                        $uop = None;
                        Filter::UnaryExpression(unop.clone(), Box::new($exp))
                    }
                    _ => $exp
                }
            };
            $stmts.push(result);
        }
    }
}

fn get_parser_expression(expr: Pair<Rule>, templates: &Vec<PatternTemplate>, errors: &mut Vec<String>) -> Filter {
    let mut stmts = Vec::new();
    let pairs = expr.into_inner();
    let mut bop: Option<BinaryOperator> = None;
    let mut uop: Option<UnaryOperator> = None;

    for pair in pairs {
        match pair.as_rule() {
            Rule::field_comparison => {
                let comp_res = get_parser_field_comparison(pair, templates);
                match comp_res {
                    Ok(comp) => handle_expr!(bop, uop, stmts, comp),
                    Err(err) => errors.push(err.to_string()),
                }
            }
            Rule::type_comparison => {
                let comp_res = get_parser_type_comparison(pair);
                match comp_res {
                    Ok(comp) => handle_expr!(bop, uop, stmts, comp),
                    Err(err) => errors.push(err.to_string()),
                }
            }
            Rule::comparison | Rule::expr => {
                handle_expr!(bop, uop, stmts, get_parser_expression(pair, templates, errors));
            }
            Rule::expr_group => {
                handle_expr!(bop, uop, stmts, Filter::Group(Box::new(get_parser_expression(pair.into_inner().next().unwrap(), templates, errors))));
            }
            Rule::not => {
                uop = Some(UnaryOperator::Not);
            }
            Rule::bool_op => {
                match get_parser_binary_op(&pair.into_inner().next().unwrap()) {
                    Ok(binop) => {
                        bop = Some(binop);
                    }
                    Err(err) => {
                        errors.push(format!("{err}"));
                    }
                }
            }
            _ => {
                errors.push(format!("did not expect rule: {pair:?}"));
            }
        }
    }
    if stmts.is_empty() {
        exit!("Invalid Filter, could not parse {errors:?}")
    }
    if stmts.len() > 1 {
        exit!("did not expect multiple rule: {stmts:?}, {errors:?}");
    }

    stmts.pop().unwrap()
}

fn get_parser_binary_op(expr: &Pair<Rule>) -> Result<BinaryOperator, M3uFilterError> {
    match expr.as_rule() {
        Rule::and => Ok(BinaryOperator::And),
        Rule::or => Ok(BinaryOperator::Or),
        _ => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown binary operator {}", expr.as_str())
    }
}

pub(crate) fn get_filter(filter_text: &str, templates: Option<&Vec<PatternTemplate>>) -> Result<Filter, M3uFilterError> {
    let empty_list = Vec::new();
    let template_list: &Vec<PatternTemplate> = templates.unwrap_or(&empty_list);
    let source  = apply_templates_to_pattern(filter_text, template_list);

    match FilterParser::parse(Rule::main, &source) {
        Ok(pairs) => {
            let mut errors = Vec::new();
            let mut result: Option<Filter> = None;
            let mut op: Option<BinaryOperator> = None;
            for pair in pairs {
                match pair.as_rule() {
                    Rule::stmt => {
                        for expr in pair.into_inner() {
                            match expr.as_rule() {
                                Rule::expr => {
                                    let expr = get_parser_expression(expr, template_list, &mut errors);
                                    match &op {
                                        Some(binop) => {
                                            result = Some(Filter::BinaryExpression(Box::new(result.unwrap()), binop.clone(), Box::new(expr)));
                                            op = None;
                                        }
                                        _ => result = Some(expr)
                                    }
                                }
                                Rule::bool_op => {
                                    match get_parser_binary_op(&expr.into_inner().next().unwrap()) {
                                        Ok(binop) => {
                                            op = Some(binop);
                                        }
                                        Err(err) => {
                                            errors.push(err.to_string());
                                        }
                                    }
                                }
                                _ => {
                                    errors.push(format!("unknown expression {expr:?}"));
                                }
                            }
                        }
                    }
                    Rule::EOI => {}
                    _ => {
                        errors.push(format!("unknown: {}", pair.as_str()));
                    }
                }
            }

            if !errors.is_empty() {
                errors.push(format!("Unable to parse filter: {}", &filter_text));
                return Err(M3uFilterError::new(M3uFilterErrorKind::Info, errors.join("\n")));
            }

            match result {
                Some(filter) => Ok(filter),
                _ => {
                    create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unable to parse filter: {}", &filter_text)
                }
            }
        }
        Err(err) => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "{}", err)
    }
}

type GraphDependency<'a> = (DiGraph<String, ()>, HashMap<usize, String>, HashMap<&'a String, Vec<String>>, bool);

fn build_dependency_graph(templates: &Vec<PatternTemplate>) -> GraphDependency {
    let regex = regex::Regex::new("!(.*?)!").unwrap();
    let mut graph = DiGraph::new();
    let mut node_ids = HashMap::new();
    let mut node_names = HashMap::new();
    let mut node_deps = HashMap::new();

    let mut add_node = |di_graph: &mut DiGraph<_, _>, node_name: &String| if let Some(idx) = node_ids.get(node_name) {
        *idx
    } else {
        let key = node_name.clone();
        let index = di_graph.add_node(node_name.clone());
        node_names.insert(index.index(), key.clone());
        node_ids.insert(key, index);
        index
    };

    for template in templates {
        let node_index = add_node(&mut graph, &template.name);
        let edges = regex.captures_iter(&template.value)
            .filter(|caps| caps.len() > 1)
            .filter_map(|caps| caps.get(1))
            .map(|caps| String::from(caps.as_str()))
            .collect::<Vec<String>>();
        let iter = edges.iter();
        for edge in iter {
            let edge_idx = add_node(&mut graph, edge);
            graph.add_edge(edge_idx, node_index, ());
        }
        node_deps.insert(&template.name, edges);
    }
    let cycles: Vec<Vec<String>> = petgraph::algo::tarjan_scc(&graph)
        .into_iter()
        .filter(|scc| scc.len() > 1)
        .map(|scc| scc.iter().map(|&i| node_names.get(&i.index()).unwrap().clone()).collect())
        .collect();
    for cyclic in &cycles {
        error!("Cyclic template dependencies detected [{}]", cyclic.join(" <-> "));
    }

    (graph, node_names, node_deps, !cycles.is_empty())
}

pub(crate) fn prepare_templates(templates: &Vec<PatternTemplate>) -> Result<Vec<PatternTemplate>, M3uFilterError> {
    let mut result: Vec<PatternTemplate> = templates.clone();
    let (graph, node_map, node_deps, cyclic) = build_dependency_graph(templates);
    if cyclic {
        return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Cyclic dependencies in templates detected!");
    }
    let mut dep_value_map: HashMap<&String, String> = templates.iter().map(|t| (&t.name, t.value.clone())).collect();
    // Perform a topological sort to get a linear ordering of the nodes
    let node_indices = toposort(&graph, None).unwrap();
    let indices = node_indices.iter();
    for node in indices {
        // only nodes with dependencies
        if graph.edges_directed(*node, petgraph::Incoming).count() > 0 {
            let node_name = node_map.get(&node.index()).unwrap();
            if let Some(deps) = node_deps.get(node_name) {
                if log_enabled!(Level::Debug) {
                    debug!("template {}  depends on [{}]", node_name, deps.join(", "));
                }
                let mut node_template = dep_value_map.get(node_name).unwrap().clone();
                for dep_name in deps {
                    let dep_template = dep_value_map.get(dep_name).unwrap().clone();
                    let new_templ = node_template.replace(format!("!{dep_name}!").as_str(), &dep_template);
                    node_template = new_templ;
                }
                dep_value_map.insert(node_name, String::from(&node_template));
                let template = result.iter_mut().find(|t| node_name.eq(&t.name)).unwrap();
                //let new_value = dep_value_map.get(&template.name).unwrap();
                template.value = String::from(&node_template);
            }
        }
    }

    if log_enabled!(Level::Debug) {
        debug!("{:#?}", result);
    }
    Ok(result)
}

pub(crate) fn apply_templates_to_pattern(pattern: &str, templates: &Vec<PatternTemplate>) -> String {
    let mut new_pattern = pattern.to_string();
    for template in templates {
        new_pattern = new_pattern.replace(format!("!{}!", &template.name).as_str(), &template.value);
    }
    new_pattern
}
