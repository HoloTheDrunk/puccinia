use std::collections::HashMap;

use {pest::Parser, pest_derive::Parser};

#[derive(Parser)]
#[grammar = "pest/config.pest"]
struct CParser;

pub struct ConfigFile {
    pub sets: HashMap<String, String>,
    pub binds: HashMap<String, String>,
}

impl ConfigFile {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let config = std::fs::read_to_string(path)?;
        let pairs = CParser::parse(Rule::config, &config)?;

        let mut sets = HashMap::new();
        let mut binds = HashMap::new();

        for pair in pairs {
            let rule = pair.as_rule();
            if rule == Rule::EOI {
                break;
            }

            let mut inner = pair.into_inner();
            let key = inner.next().unwrap().as_str().to_string();
            let value = inner.next().unwrap().as_str().to_string();

            match rule {
                Rule::set => sets.insert(key, value),
                Rule::bind => binds.insert(key, value),
                _ => unreachable!(),
            };
        }

        Ok(Self { sets, binds })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_empty_string() {
        CParser::parse(Rule::config, "").unwrap();
    }

    #[test]
    fn parse_set() {
        CParser::parse(Rule::config, "set foo bar;").unwrap();
    }

    #[test]
    fn parse_bind() {
        CParser::parse(Rule::config, "bind foo <Escape>;").unwrap();
        CParser::parse(Rule::config, "bind foo ^b;").unwrap();
    }

    #[test]
    fn parse_default() {
        let default_config = std::fs::read_to_string("assets/.puccirc").unwrap();
        CParser::parse(Rule::config, &default_config).unwrap();
    }

    #[test]
    fn load_default() {
        ConfigFile::load(std::path::Path::new("assets/.puccirc")).unwrap();
    }
}
