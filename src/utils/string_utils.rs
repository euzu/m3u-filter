// other implementations like calculating text_distance on all titles took too much time
// we keep it now as simple as possible and less memory intensive.
pub fn get_title_group(text: &str) -> String {
    let alphabetic_only: String = text.chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    let parts = alphabetic_only.split_whitespace();
    let mut combination = String::new();
    for p in parts {
        combination = format!("{combination} {p}").trim().to_string();
        if combination.len() > 2 {
            return combination;
        }
    }
    text.to_string()
}

pub trait Capitalize {
    fn capitalize(&self) -> String;
}

// Implement the Capitalize trait for &str
impl Capitalize for &str {
    fn capitalize(&self) -> String {
        let mut chars = self.chars();
        chars.next().map_or_else(String::new, |first_char| first_char.to_uppercase().collect::<String>() + chars.as_str())
    }
}

// Implement the trait for String as well
impl Capitalize for String {
    fn capitalize(&self) -> String {
        self.as_str().capitalize()  // Reuse the &str implementation
    }
}