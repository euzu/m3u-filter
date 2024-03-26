// other implementations like calculating text_distance on all titles took too much time
// we keep it now as simple as possible and less memory intensive.
pub (crate) fn get_title_group(text: &str) -> String {
    let alphabetic_only: String = text.chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    let parts = alphabetic_only.split_whitespace();
    let mut combination = "".to_string();
    for p in parts.into_iter() {
        combination = format!("{} {}", combination, p).trim().to_string();
        if combination.len() > 2 {
            return combination;
        }
    }
    text.to_string()
}
