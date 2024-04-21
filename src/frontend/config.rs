use std::{collections::HashMap, path::Path};

use pest::iterators::Pairs;

use super::ArgType;

use {pest::Parser, pest_derive::Parser};

#[derive(Parser)]
#[grammar = "pest/config.pest"]
struct CParser;

pub struct ConfigFile {
    pub sets: HashMap<String, String>,
    pub binds: HashMap<String, String>,
    pub commands: Vec<CommandBinding>,
}

#[derive(Clone, Debug)]
pub struct CommandBinding {
    pub bind: String,
    pub name: String,
    pub args: Vec<CommandBindingArg>,
    pub description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CommandBindingArg {
    pub name: String,
    pub arg_type: ArgType,
    pub description: Option<String>,
}

impl ConfigFile {
    pub fn load(config: &str) -> anyhow::Result<Self> {
        let pairs = CParser::parse(Rule::config, &config)?;

        let mut sets = HashMap::new();
        let mut binds = HashMap::new();
        let mut commands = Vec::new();

        for pair in pairs {
            let rule = pair.as_rule();
            if rule == Rule::EOI {
                break;
            }

            let inner = pair.into_inner();

            match rule {
                Rule::set | Rule::bind => {
                    let (key, value) = Self::parse_key_value(inner);
                    let target = (rule == Rule::set)
                        .then_some(&mut sets)
                        .unwrap_or(&mut binds);
                    target.insert(key, value);
                }
                Rule::command => commands.push(Self::parse_command(inner)),
                _ => unreachable!(),
            };
        }

        Ok(Self {
            sets,
            binds,
            commands,
        })
    }

    fn parse_key_value(mut pairs: Pairs<Rule>) -> (String, String) {
        let key = pairs.next().unwrap().as_str().to_string();
        let value = pairs.next().unwrap().as_str().to_string();
        (key, value)
    }

    fn parse_command(mut pairs: Pairs<Rule>) -> CommandBinding {
        let bind = pairs.next().unwrap().as_str().to_string();
        let name = pairs.next().unwrap().as_str().to_string();
        let mut args = Vec::new();
        let mut description = None;

        for pair in pairs {
            let rule = pair.as_rule();

            match rule {
                Rule::command_arg => args.push(Self::parse_command_arg(pair.into_inner())),
                Rule::command_description => description = Some(pair.as_str().to_string()),
                _ => unreachable!("Parsing grammar error"),
            }
        }

        CommandBinding {
            bind,
            name,
            args,
            description,
        }
    }

    fn parse_command_arg(mut pairs: Pairs<Rule>) -> CommandBindingArg {
        let name = pairs.next().unwrap().as_str().to_string();

        let type_pair = pairs.next().unwrap();
        let type_str = type_pair.as_str();

        let arg_type = match type_str {
            "Number" => ArgType::Number,
            "Boolean" => ArgType::Boolean,
            "String" | _ => ArgType::String,
        };

        let description = pairs.next().map(|p| p.as_str().to_string());

        CommandBindingArg {
            name,
            arg_type,
            description,
        }
    }
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self::load(include_str!("../../assets/config/config.pucci")).unwrap()
    }
}

impl TryFrom<&Path> for ConfigFile {
    type Error = anyhow::Error;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        let config = std::fs::read_to_string(value)?;
        Self::load(&config)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn parse_config(config: &str) {
        CParser::parse(Rule::config, config).unwrap();
    }

    #[test]
    fn parse_empty_string() {
        parse_config("");
    }

    #[test]
    fn parse_set() {
        parse_config("set foo bar");
    }

    #[test]
    fn parse_bind() {
        parse_config("bind foo <Escape>");
        parse_config("bind foo ^b");
    }

    #[test]
    fn parse_command() {
        parse_config("command foo = bar");
        parse_config("command foo = bar: Description !command");
        parse_config("command foo = bar: @baz: Any@ !command");
    }

    #[test]
    fn parse_default() {
        CParser::parse(
            Rule::config,
            include_str!("../../assets/config/config.pucci"),
        )
        .unwrap();
    }
}
