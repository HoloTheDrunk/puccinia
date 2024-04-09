use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "pest/config.pest"]
struct CParser;

#[cfg(test)]
mod test {
    use pest::Parser;

    use crate::config::*;

    #[test]
    fn parse_empty_string() {
        CParser::parse(Rule::config, "").unwrap();
    }

    #[test]
    fn parse_set() {
        CParser::parse(Rule::config, "set foo bar;").unwrap();
    }

    #[test]
    fn parse_unset() {
        CParser::parse(Rule::config, "unset foo;").unwrap();
    }

    #[test]
    fn parse_bind() {
        CParser::parse(Rule::config, "bind foo <Escape>;").unwrap();
        CParser::parse(Rule::config, "bind foo ^b;").unwrap();
    }

    #[test]
    fn parse_unbind() {
        CParser::parse(Rule::config, "unbind foo;").unwrap();
    }

    #[test]
    fn parse_default() {
        let default_config = std::fs::read_to_string("assets/.puccirc").unwrap();
        CParser::parse(Rule::config, &default_config).unwrap();
    }
}
