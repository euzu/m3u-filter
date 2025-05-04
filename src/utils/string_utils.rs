use rand::Rng;

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

pub fn get_trimmed_string(value: &Option<String>) -> Option<String> {
    if let Some(v) = value {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

pub fn generate_random_string(length: usize) -> String {
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();

    let random_string: String = (0..length)
        .map(|_| {
            let idx = rng.random_range(0..charset.len());
            charset[idx] as char
        })
        .collect();

    random_string
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use crate::utils::generate_random_string;

    #[test]
    fn test_generate_random_string() {
        let mut strings = HashSet::new();
        for _i in 0..100 {
            strings.insert(generate_random_string(5));
        }
        assert_eq!(strings.len(), 100);
    }
}