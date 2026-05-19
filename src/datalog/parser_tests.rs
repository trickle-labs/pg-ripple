//! Tests for the Datalog parser (extracted from parser.rs v0.122.0 H17-02).

use super::*;

#[test]
fn test_tokenize_simple() {
    let text = "?x <p> ?y :- ?x <q> ?z . ?a <b> ?c :- ?d <e> ?f .";
    let rules = tokenize_rules(text);
    assert_eq!(rules.len(), 2);
}

#[test]
fn test_tokenize_with_literal() {
    let text = r#"?x <p> "hello.world" :- ?x <q> ?z ."#;
    let rules = tokenize_rules(text);
    assert_eq!(rules.len(), 1, "dot inside literal should not split");
}

#[test]
fn test_tokenize_comment() {
    let text = "# this is a comment\n?x <p> ?y :- ?x <q> ?z .";
    let rules = tokenize_rules(text);
    assert_eq!(rules.len(), 1);
}

#[test]
fn test_find_neck() {
    let rule = "?x <p> ?y :- ?x <q> ?z";
    let pos = find_neck(rule).unwrap();
    assert_eq!(&rule[pos..pos + 2], ":-");
}

#[test]
fn test_parse_comparison() {
    let lit = "?a > 18";
    let result = try_parse_comparison(lit);
    assert!(result.is_some());
    if let Some(BodyLiteral::Compare(_, op, _)) = result {
        assert_eq!(op, CompareOp::Gt);
    }
}

#[test]
fn test_split_body_simple() {
    let body = "?x <p> ?y, ?y <q> ?z";
    let parts = split_body(body);
    assert_eq!(parts.len(), 2);
}
