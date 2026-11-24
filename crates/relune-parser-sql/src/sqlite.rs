//! `SQLite` syntax missing from sqlparser's built-in dialect.
//!
//! Keep the built-in dialect identity and delegate its capabilities. These
//! adapters preserve expressions in the AST; they do not rewrite source text
//! or discard a table when one of its CHECK expressions uses `SQLite` syntax.

use std::any::TypeId;

use sqlparser::ast::{BinaryOperator, Expr, Statement, Value};
use sqlparser::dialect::{Dialect, SQLiteDialect};
use sqlparser::keywords::Keyword;
use sqlparser::parser::{Parser, ParserError};
use sqlparser::tokenizer::Token;

#[derive(Debug)]
pub(crate) struct SchemaSqliteDialect;

impl Dialect for SchemaSqliteDialect {
    fn dialect(&self) -> TypeId {
        SQLiteDialect {}.dialect()
    }

    fn is_delimited_identifier_start(&self, ch: char) -> bool {
        SQLiteDialect {}.is_delimited_identifier_start(ch)
    }

    fn identifier_quote_style(&self, identifier: &str) -> Option<char> {
        SQLiteDialect {}.identifier_quote_style(identifier)
    }

    fn is_identifier_start(&self, ch: char) -> bool {
        SQLiteDialect {}.is_identifier_start(ch)
    }

    fn is_identifier_part(&self, ch: char) -> bool {
        SQLiteDialect {}.is_identifier_part(ch)
    }

    fn supports_filter_during_aggregation(&self) -> bool {
        SQLiteDialect {}.supports_filter_during_aggregation()
    }

    fn supports_start_transaction_modifier(&self) -> bool {
        SQLiteDialect {}.supports_start_transaction_modifier()
    }

    fn supports_in_empty_list(&self) -> bool {
        SQLiteDialect {}.supports_in_empty_list()
    }

    fn supports_limit_comma(&self) -> bool {
        SQLiteDialect {}.supports_limit_comma()
    }

    fn supports_asc_desc_in_column_definition(&self) -> bool {
        SQLiteDialect {}.supports_asc_desc_in_column_definition()
    }

    fn supports_dollar_placeholder(&self) -> bool {
        SQLiteDialect {}.supports_dollar_placeholder()
    }

    fn supports_notnull_operator(&self) -> bool {
        SQLiteDialect {}.supports_notnull_operator()
    }

    fn supports_comma_separated_trim(&self) -> bool {
        SQLiteDialect {}.supports_comma_separated_trim()
    }

    fn supports_numeric_literal_underscores(&self) -> bool {
        SQLiteDialect {}.supports_numeric_literal_underscores()
    }

    fn parse_statement(&self, parser: &mut Parser<'_>) -> Option<Result<Statement, ParserError>> {
        if parser.parse_keyword(Keyword::PRAGMA) {
            Some(parse_pragma(parser))
        } else {
            SQLiteDialect {}.parse_statement(parser)
        }
    }

    fn parse_infix(
        &self,
        parser: &mut Parser<'_>,
        expr: &Expr,
        precedence: u8,
    ) -> Option<Result<Expr, ParserError>> {
        for keyword in [Keyword::GLOB, Keyword::MATCH, Keyword::REGEXP] {
            if parser.parse_keywords(&[Keyword::NOT, keyword]) {
                return Some(binary(parser, expr, precedence, &format!("NOT {keyword}")));
            }
        }

        // SQLite permits any expression on the right of IS / IS NOT. Keep
        // the standard spellings (NULL, TRUE, DISTINCT FROM, ...) on the base
        // parser path, including its special handling of boolean predicates.
        if matches!(parser.peek_token().token, Token::Word(w) if w.keyword == Keyword::IS) {
            let mut offset = 1;
            let negated = matches!(parser.peek_nth_token(offset).token, Token::Word(w) if w.keyword == Keyword::NOT);
            if negated {
                offset += 1;
            }
            let standard = matches!(parser.peek_nth_token(offset).token, Token::Word(w)
                if matches!(w.keyword, Keyword::NULL | Keyword::TRUE | Keyword::FALSE | Keyword::DISTINCT | Keyword::UNKNOWN));
            if !standard {
                parser.next_token();
                if negated {
                    parser.next_token();
                }
                return Some(binary(
                    parser,
                    expr,
                    precedence,
                    if negated { "IS NOT" } else { "IS" },
                ));
            }
        }
        SQLiteDialect {}.parse_infix(parser, expr, precedence)
    }
}

fn binary(
    parser: &mut Parser<'_>,
    left: &Expr,
    precedence: u8,
    operator: &str,
) -> Result<Expr, ParserError> {
    Ok(Expr::BinaryOp {
        left: Box::new(left.clone()),
        op: BinaryOperator::Custom(operator.into()),
        right: Box::new(parser.parse_subexpr(precedence)?),
    })
}

// The method item is not lifetime-general enough for Parser::try_parse.
#[allow(clippy::redundant_closure_for_method_calls)]
fn parse_pragma(parser: &mut Parser<'_>) -> Result<Statement, ParserError> {
    // Retain the built-in handling of strings, numbers and placeholders.
    if let Ok(statement) = parser.try_parse(|parser| parser.parse_pragma()) {
        return Ok(statement);
    }
    // SQLite also accepts bare words, including ON/OFF and journal modes.
    let name = parser.parse_object_name(false)?;
    let is_eq = parser.consume_token(&Token::Eq);
    if !is_eq {
        parser.expect_token(&Token::LParen)?;
    }
    let token = parser.next_token();
    let Token::Word(word) = token.token else {
        return Err(ParserError::ParserError(format!(
            "Expected PRAGMA value, found {token}"
        )));
    };
    if !is_eq {
        parser.expect_token(&Token::RParen)?;
    }
    Ok(Statement::Pragma {
        name,
        value: Some(Value::SingleQuotedString(word.value).with_span(token.span)),
        is_eq,
    })
}
