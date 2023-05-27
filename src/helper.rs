// utility?
use std::env;

use regex::Regex;

pub fn replace_str(  // render template? ( I think we've discussed this?
    to_replace: &str,  // to_be_replaced? (I actually prefer simply `s`, which does not cause any ambiguity)
    values_table: std::collections::HashMap<String, String>,  // `assignments`?
) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let mut path = String::new();
    let mut i = 0;
    while i < to_replace.len() {
        match to_replace[i..].find("{{") { // why using regex below, but here do it manually?
            Some(start) => {
                path.push_str(&to_replace[i..i + start]);
                let end = to_replace[i + start + 2..].find("}}").unwrap() + i + start + 2;
                let var_string = &to_replace[i + start + 2..end].trim();
                let substring_start = var_string.find('[');
                match substring_start {
                    Some(substring_start) => {
                        let substring_end = var_string.find(']').unwrap();
                        let var_name = &var_string[..substring_start];
                        let indexes = &var_string[substring_start + 1..substring_end];
                        let values = values_table.get(var_name).unwrap();  // always try to avoid naming like `values`
                        let values = values.chars().collect::<Vec<char>>();
                        if indexes.is_empty() {
                            path.push_str(&values.iter().collect::<String>());
                        } else if indexes.contains("..") {
                            let mut indexes = indexes.split("..");
                            let start = indexes.next().unwrap().parse::<usize>().unwrap_or(0);
                            let end = indexes
                                .next()
                                .unwrap_or("")
                                .parse::<usize>()
                                .unwrap_or(values.len());
                            path.push_str(&values[start..end].iter().collect::<String>());
                        } else {
                            let index = indexes.parse::<usize>().unwrap();
                            path.push(values[index]);
                        }
                        i = end + 2;
                    }
                    None => {
                        let var_name = &var_string[..];
                        let values = values_table.get(var_name).unwrap();
                        path.push_str(&values[..]);
                        i = end + 2;
                    }
                }
            }
            None => {
                path.push_str(&to_replace[i..]);
                break;
            }
        }
    }
    Ok(path.to_string())
}

pub fn replace_env_var(
    to_replace: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let re = Regex::new(r"\$(\w+)").unwrap();
    let replaced_path = re.replace_all(to_replace, |caps: &regex::Captures| {
        let var_name = &caps[1];
        match env::var(var_name) {
            Ok(val) => val,
            Err(_) => caps[0].to_string(),
        }
    });
    Ok(replaced_path.to_string())
}
