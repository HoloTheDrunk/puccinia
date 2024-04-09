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
        CParser::parse(Rule::config, r#"set foo "bar";"#).unwrap();
        CParser::parse(Rule::config, r#"set foo "";"#).unwrap();
    }

    #[test]
    fn parse_unset() {
        CParser::parse(Rule::config, "unset foo;").unwrap();
    }

    #[test]
    fn parse_toggle() {
        CParser::parse(Rule::config, "toggle foo;").unwrap();
    }

    #[test]
    fn parse_bind() {
        CParser::parse(Rule::config, r#"bind foo "Escape";"#).unwrap();
        CParser::parse(Rule::config, r#"bind foo "<C-b>";"#).unwrap();
    }

    #[test]
    fn parse_unbind() {
        CParser::parse(Rule::config, "unbind foo;").unwrap();
    }
}
