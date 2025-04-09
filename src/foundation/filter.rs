#![allow(clippy::empty_docs)]

use std::collections::HashMap;
use enum_iterator::all;
use log::{debug, error, log_enabled, trace, Level};
use pest::iterators::Pair;
use pest::Parser;

use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::ItemField;
use crate::model::playlist::{PlaylistItem, PlaylistItemType};
use crate::tools::directed_graph::DirectedGraph;
use crate::m3u_filter_error::{create_m3u_filter_error_result, info_err};
use crate::utils::sys_utils::exit;

pub fn get_field_value(pli: &PlaylistItem, field: &ItemField) -> String {
    let header = &pli.header;
    let value = match field {
        ItemField::Group => header.group.to_string(),
        ItemField::Name => header.name.to_string(),
        ItemField::Title => header.title.to_string(),
        ItemField::Url => header.url.to_string(),
        ItemField::Input => header.input_name.to_string(),
        ItemField::Type => header.item_type.to_string(),
    };
    value.to_string()
}

pub fn set_field_value(pli: &mut PlaylistItem, field: &ItemField, value: String) {
    let header = &mut pli.header;
    match field {
        ItemField::Group => header.group = value,
        ItemField::Name => header.name = value,
        ItemField::Title => header.title = value,
        ItemField::Url => header.url = value,
        ItemField::Input => header.input_name = value,
        ItemField::Type => {}
    };
}

pub struct ValueProvider<'a> {
    pub pli: &'a PlaylistItem,
}

impl ValueProvider<'_> {
    fn call(&self, field: &ItemField) -> String {
        get_field_value(self.pli, field)
    }
}

pub trait ValueProcessor {
    fn process(&mut self, field: &ItemField, value: &str, rewc: &RegexWithCaptures) -> bool;
}

pub struct MockValueProcessor {}

impl ValueProcessor for MockValueProcessor {
    fn process(&mut self, _: &ItemField, _: &str, _: &RegexWithCaptures) -> bool {
        false
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PatternTemplate {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct RegexWithCaptures {
    pub restr: String,
    pub re: regex::Regex,
    pub captures: Vec<String>,
}

#[derive(Parser)]
#[grammar_inline = r#"
WHITESPACE = _{ " " | "\t" | "\r" | "\n"}
field = { ^"group" | ^"title" | ^"name" | ^"url" | ^"input"}
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
pub enum UnaryOperator {
    Not,
}

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    And,
    Or,
}

impl BinaryOperator {
    const OP_OR: &'static str = "OR";
    const OP_AND: &'static str = "AND";
}

impl std::fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Self::Or => Self::OP_OR,
            Self::And => Self::OP_AND,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Filter {
    Group(Box<Filter>),
    FieldComparison(ItemField, RegexWithCaptures),
    TypeComparison(ItemField, PlaylistItemType),
    UnaryExpression(UnaryOperator, Box<Filter>),
    BinaryExpression(Box<Filter>, BinaryOperator, Box<Filter>),
}

impl Filter {
    pub fn filter(&self, provider: &ValueProvider, processor: &mut dyn ValueProcessor) -> bool {
        match self {
            Self::FieldComparison(field, rewc) => {
                let value = provider.call(field);
                let is_match = rewc.re.is_match(value.as_str());
                if log_enabled!(Level::Trace) {
                    if is_match {
                        debug!("Match found: {:?} {} => {}={}", &rewc, &rewc.restr, &field, &value);
                    } else {
                        debug!("Match failed: {self}: {:?} {} => {}={}", &rewc, &rewc.restr, &field, &value);
                    }
                }
                if is_match {
                    processor.process(field, &value, rewc);
                }
                is_match
            }
            Self::TypeComparison(field, item_type) => {
                let value = provider.call(field);
                get_filter_item_type(value.as_str()).is_some_and(|pli_type| {
                    let is_match = pli_type.eq(item_type);
                    if log_enabled!(Level::Trace) {
                        if is_match {
                            debug!("Match found: {:?} {}", &field, value);
                        } else {
                            debug!("Match failed: {self}: {:?} {}", &field, &value);
                        }
                    }
                    is_match
                })
            }
            Self::Group(expr) => expr.filter(provider, processor),
            Self::UnaryExpression(op, expr) => match op {
                UnaryOperator::Not => !expr.filter(provider, processor),
            },
            Self::BinaryExpression(left, op, right) => match op {
                BinaryOperator::And => {
                    left.filter(provider, processor) && right.filter(provider, processor)
                }
                BinaryOperator::Or => {
                    left.filter(provider, processor) || right.filter(provider, processor)
                }
            },
        }
    }
}

impl Filter {
    const LIVE: &'static str = "live";
    const VOD: &'static str = "vod";
    const SERIES: &'static str = "series";
    const UNSUPPORTED: &'static str = "unsupported";
}

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::FieldComparison(field, rewc) => {
                write!(f, "{} ~ \"{}\"", field, String::from(&rewc.restr))
            }
            Self::TypeComparison(field, item_type) => {
                write!(f, "{} = {}", field, match item_type {
                    PlaylistItemType::Live => Self::LIVE,
                    PlaylistItemType::Video => Self::VOD,
                    PlaylistItemType::Series | PlaylistItemType::SeriesInfo => Self::SERIES, // yes series-info is handled as series in filter
                    _ => Self::UNSUPPORTED
                })
            }
            Self::Group(stmt) => {
                write!(f, "({stmt})")
            }
            Self::UnaryExpression(op, expr) => {
                let flt = match op {
                    UnaryOperator::Not => format!("NOT {expr}"),
                };
                write!(f, "{flt}")
            }
            Self::BinaryExpression(left, op, right) => {
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

fn get_parser_regexp(
    expr: &Pair<Rule>,
    templates: &Vec<PatternTemplate>,
) -> Result<RegexWithCaptures, M3uFilterError> {
    if expr.as_rule() == Rule::regexp {
        let mut parsed_text = String::from(expr.as_str());
        parsed_text.pop();
        parsed_text.remove(0);
        let regstr = apply_templates_to_pattern(&parsed_text, templates);
        let re = regex::Regex::new(regstr.as_str());
        if re.is_err() {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {}", regstr);
        }
        let regexp = re.unwrap();
        let captures = regexp
            .capture_names()
            .flatten()
            .map(String::from)
            .filter(|x| !x.is_empty())
            .collect::<Vec<String>>();
        if log_enabled!(Level::Trace) {
            trace!("Created regex: {} with captures: [{}]", regstr, captures.join(", "));
        }
        return Ok(RegexWithCaptures {
            restr: regstr,
            re: regexp,
            captures,
        });
    }
    create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "unknown field: {}", expr.as_str())
}

fn get_parser_field_comparison(
    expr: Pair<Rule>,
    templates: &Vec<PatternTemplate>,
) -> Result<Filter, M3uFilterError> {
    let mut expr_inner = expr.into_inner();
    match get_parser_item_field(&expr_inner.next().unwrap()) {
        Ok(field) => match get_parser_regexp(&expr_inner.next().unwrap(), templates) {
            Ok(regexp) => Ok(Filter::FieldComparison(field, regexp)),
            Err(err) => Err(err),
        },
        Err(err) => Err(err),
    }
}

fn get_filter_item_type(text_item_type: &str) -> Option<PlaylistItemType> {
    if text_item_type.eq_ignore_ascii_case("live") {
        Some(PlaylistItemType::Live)
    } else if text_item_type.eq_ignore_ascii_case("movie")
        || text_item_type.eq_ignore_ascii_case("video")
        || text_item_type.eq_ignore_ascii_case("vod")
    {
        Some(PlaylistItemType::Video)
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
    item_type.map_or_else(|| create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse item type: {text_item_type}"), |itype| Ok(Filter::TypeComparison(ItemField::Type, itype)))
}

macro_rules! handle_expr {
    ($bop: expr, $uop: expr, $stmts: expr, $exp: expr) => {{
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
                _ => $exp,
            },
        };
        $stmts.push(result);
    }};
}

fn get_parser_expression(
    expr: Pair<Rule>,
    templates: &Vec<PatternTemplate>,
    errors: &mut Vec<String>,
) -> Filter {
    let mut stmts = Vec::with_capacity(128);
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
            Rule::bool_op => match get_parser_binary_op(&pair.into_inner().next().unwrap()) {
                Ok(binop) => {
                    bop = Some(binop);
                }
                Err(err) => {
                    errors.push(format!("{err}"));
                }
            },
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
        _ => create_m3u_filter_error_result!(
            M3uFilterErrorKind::Info,
            "Unknown binary operator {}",
            expr.as_str()
        ),
    }
}

pub fn get_filter(
    filter_text: &str,
    templates: Option<&Vec<PatternTemplate>>,
) -> Result<Filter, M3uFilterError> {
    let empty_list = Vec::with_capacity(0);
    let template_list: &Vec<PatternTemplate> = templates.unwrap_or(&empty_list);
    let source = apply_templates_to_pattern(filter_text, template_list);

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
                                    let expr =
                                        get_parser_expression(expr, template_list, &mut errors);
                                    match &op {
                                        Some(binop) => {
                                            result = Some(Filter::BinaryExpression(
                                                Box::new(result.unwrap()),
                                                binop.clone(),
                                                Box::new(expr),
                                            ));
                                            op = None;
                                        }
                                        _ => result = Some(expr),
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
                return Err(info_err!(errors.join("\n")));
            }

            result.map_or_else(
                || {
                    create_m3u_filter_error_result!(
                        M3uFilterErrorKind::Info,
                        "Unable to parse filter: {}",
                        &filter_text
                    )
                },
                Ok,
            )
        }
        Err(err) => create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "{}", err),
    }
}

fn build_dependency_graph(
    templates: &Vec<PatternTemplate>,
) -> Result<DirectedGraph<String>, M3uFilterError> {
    let mut graph = DirectedGraph::<String>::new();
    let re_template = regex::Regex::new("!(.*?)!").unwrap();
    for template in templates {
        graph.add_node(&template.name);
        re_template
            .captures_iter(&template.value)
            .filter(|caps| caps.len() > 1)
            .filter_map(|caps| caps.get(1))
            .map(|caps| String::from(caps.as_str()))
            .for_each(|e| {
                graph.add_node(&e);
                graph.add_edge(&template.name, &e);
            });
    }
    let cycles = graph.find_cycles();
    for cyclic in &cycles {
        error!(
            "Cyclic template dependencies detected [{}]",
            cyclic.join(" <-> ")
        );
    }
    if !cycles.is_empty() {
        return create_m3u_filter_error_result!(
            M3uFilterErrorKind::Info,
            "Cyclic dependencies in templates detected!"
        );
    }
    Ok(graph)
}

pub fn prepare_templates(
    templates: &Vec<PatternTemplate>,
) -> Result<Vec<PatternTemplate>, M3uFilterError> {
    let graph = build_dependency_graph(templates)?;
    let mut template_values = HashMap::<String, String>::new();
    let mut template_map: HashMap<String, PatternTemplate> = templates
        .iter()
        .map(|item| {
            template_values.insert(item.name.clone(), item.value.clone());
            (item.name.clone(), item.clone())
        })
        .collect();

    if let Some(dependencies) = graph.get_dependencies() {
        if let Some(sorted) = graph.topological_sort() {
            for template_name in sorted {
                if let Some(depends_on) = dependencies.get(&template_name) {
                    let mut templ_value = template_values.get(&template_name).unwrap().to_string();
                    for dep_templ_name in depends_on {
                        let dep_value = template_values.get(dep_templ_name).ok_or_else(|| M3uFilterError::new(M3uFilterErrorKind::Info, format!("Failed to load template {dep_templ_name}")))?;
                        templ_value =
                            templ_value.replace(format!("!{dep_templ_name}!").as_str(), dep_value);
                    }
                    template_values.insert(template_name.clone(), templ_value);
                }
            }

            for (k, v) in template_values {
                let template = template_map.get_mut(&k).unwrap();
                template.value = v;
            }
        }
    }
    Ok(template_map.into_values().collect())
}

pub fn apply_templates_to_pattern(pattern: &str, templates: &Vec<PatternTemplate>) -> String {
    let mut new_pattern = pattern.to_string();
    for template in templates {
        new_pattern =
            new_pattern.replace(format!("!{}!", &template.name).as_str(), &template.value);
    }
    new_pattern
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    use crate::foundation::filter::{get_filter, MockValueProcessor, ValueProvider};
    use crate::model::playlist::{PlaylistItem, PlaylistItemHeader};

    fn create_mock_pli(name: &str, group: &str) -> PlaylistItem {
        PlaylistItem {
            header: PlaylistItemHeader {
                name: name.to_string(),
                group: group.to_string(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_filter_1() {
        let flt1 = r#"(Group ~ "A" OR Group ~ "B") AND (Name ~ "C" OR Name ~ "D" OR Name ~ "E") OR (NOT (Title ~ "F") AND NOT Title ~ "K")"#;
        match get_filter(flt1, None) {
            Ok(filter) => {
                assert_eq!(format!("{filter}"), flt1);
            }
            Err(e) => {
                panic!("{}", e)
            }
        }
    }

    #[test]
    fn test_filter_2() {
        let flt2 = r#"Group ~ "d" AND ((Name ~ "e" AND NOT ((Name ~ "c" OR Name ~ "f"))) OR (Name ~ "a" OR Name ~ "b"))"#;
        match get_filter(flt2, None) {
            Ok(filter) => {
                assert_eq!(format!("{filter}"), flt2);
            }
            Err(e) => {
                panic!("{}", e)
            }
        }
    }

    #[test]
    fn test_filter_3() {
        let flt = r#"Group ~ "d" AND ((Name ~ "e" AND NOT ((Name ~ "c" OR Name ~ "f"))) OR (Name ~ "a" OR Name ~ "b")) AND (Type = vod)"#;
        match get_filter(flt, None) {
            Ok(filter) => {
                assert_eq!(format!("{filter}"), flt);
            }
            Err(e) => {
                panic!("{}", e)
            }
        }
    }

    #[test]
    fn test_filter_4() {
        let flt = r#"NOT (Name ~ ".*24/7.*" AND Group ~ "^US.*")"#;
        match get_filter(flt, None) {
            Ok(filter) => {
                assert_eq!(format!("{filter}"), flt);
                let channels = vec![
                    create_mock_pli("24/7: Cars", "FR Channels"),
                    create_mock_pli("24/7: Cars", "US Channels"),
                    create_mock_pli("Entertainment", "US Channels"),
                ];
                let mut processor = MockValueProcessor {};
                let filtered: Vec<&PlaylistItem> = channels
                    .iter()
                    .filter(|&chan| {
                        let provider = ValueProvider {
                            pli: chan,
                        };
                        filter.filter(&provider, &mut processor)
                    })
                    .collect();
                assert_eq!(filtered.len(), 2);
                assert_eq!(
                    filtered.iter().any(|&chan| {
                        let group = chan.header.group.to_string();
                        let name = chan.header.name.to_string();
                        name.eq("24/7: Cars") && group.eq("FR Channels")
                    }),
                    true
                );
                assert_eq!(
                    filtered.iter().any(|&chan| {
                        let group = chan.header.group.to_string();
                        let name = chan.header.name.to_string();
                        name.eq("Entertainment") && group.eq("US Channels")
                    }),
                    true
                );
                assert_eq!(
                    filtered.iter().any(|&chan| {
                        let group = chan.header.group.to_string();
                        let name = chan.header.name.to_string();
                        name.eq("24/7: Cars") && group.eq("US Channels")
                    }),
                    false
                );
            }
            Err(e) => {
                panic!("{}", e)
            }
        }
    }

    #[test]
    fn test_filter_5() {
        let flt = r#"NOT (Name ~ "NC" OR Group ~ "GA") AND (Name ~ "NA" AND Group ~ "GA") OR (Name ~ "NB" AND Group ~ "GB")"#;
        match get_filter(flt, None) {
            Ok(filter) => {
                assert_eq!(format!("{filter}"), flt);
                let channels = vec![
                    create_mock_pli("NA", "GA"),
                    create_mock_pli("NB", "GB"),
                    create_mock_pli("NA", "GB"),
                    create_mock_pli("NB", "GA"),
                    create_mock_pli("NC", "GA"),
                    create_mock_pli("NA", "GC"),
                ];
                let mut processor = MockValueProcessor {};
                let filtered: Vec<&PlaylistItem> = channels
                    .iter()
                    .filter(|&chan| {
                        let provider = ValueProvider {
                            pli: chan,
                        };
                        filter.filter(&provider, &mut processor)
                    })
                    .collect();
                assert_eq!(filtered.len(), 1);
            }
            Err(e) => {
                panic!("{}", e)
            }
        }
    }

    #[test]
    fn test_filter_6() {
        let flt = r####"
            Group ~ "^EU \| FRANCE.*"
            OR  Input ~ "hello"
            OR  Group ~ "^VOD \| FR.*"
            OR  Group ~ "\[FR\].*"
            OR  Group ~ "^SRS \| FR.*"
            AND NOT (Group ~ ".* LQ.*"
            OR Title ~ ".* LQ.*"
            OR Group ~ ".* SD.*"
            OR Title ~ ".* SD.*"
            OR Group ~ ".* HD.*"
            OR Title ~ ".* HD.*"
            OR Group ~ "(?i).*sport.*"
            OR Group ~ "(?i).*DAZN.*"
            OR Group ~ "(?i).*EQUIPE.*"
            OR Group ~ "DOM TOM.*"
            OR Group ~ "(?i).*PLUTO.*"
            OR Title ~ "(?i).*GOLD.*"
            OR Title ~ "###.*")"####;

        match get_filter(flt, None) {
            Ok(filter) => {
                let re = Regex::new(r"\s+").unwrap();
                let result = re.replace_all(&flt, " ");
                assert_eq!(format!("{filter}"), result.trim());
            }
            Err(e) => {
                panic!("{}", e)
            }
        }
    }
}
