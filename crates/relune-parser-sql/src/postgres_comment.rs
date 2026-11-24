//! `PostgreSQL` comment syntax not yet handled by sqlparser's COMMENT parser.

use sqlparser::ast::{CommentObject, Statement};
use sqlparser::keywords::Keyword;
use sqlparser::parser::{Parser, ParserError};
use sqlparser::tokenizer::Token;

// The closure is needed for try_parse's higher-ranked parser lifetime.
#[allow(clippy::redundant_closure_for_method_calls)]
pub(crate) fn parse_comment(parser: &mut Parser<'_>) -> Result<Statement, ParserError> {
    let mut statement = match parser.try_parse(|parser| parser.parse_statement()) {
        Ok(statement) => statement,
        Err(original_error) => {
            // The AST has no routine signature field. Consume the signature
            // here, retaining the object kind so the schema layer emits its
            // usual unsupported-routine-comment diagnostic. Table and column
            // comments continue through the built-in parser above.
            if !parser.parse_keywords(&[Keyword::COMMENT, Keyword::ON]) {
                return Err(original_error);
            }
            let object_type = if parser.parse_keyword(Keyword::FUNCTION) {
                CommentObject::Function
            } else if parser.parse_keyword(Keyword::PROCEDURE) {
                CommentObject::Procedure
            } else {
                return Err(original_error);
            };
            let object_name = parser.parse_object_name(false)?;
            if parser.consume_token(&Token::LParen) {
                skip_signature(parser)?;
            }
            parser.expect_keyword_is(Keyword::IS)?;
            let comment = if parser.parse_keyword(Keyword::NULL) {
                None
            } else {
                if matches!(parser.peek_token().token, Token::SemiColon | Token::EOF) {
                    return Err(ParserError::ParserError(
                        "Expected routine comment string or NULL".into(),
                    ));
                }
                Some(parser.parse_literal_string()?)
            };
            Statement::Comment {
                object_type,
                object_name,
                comment,
                if_exists: false,
            }
        }
    };

    // PostgreSQL joins adjacent string literals only when separated by a
    // newline. Token spans preserve that distinction (including comments in
    // the whitespace) without touching SQL source text or quoted semicolons.
    if let Statement::Comment {
        comment: Some(comment),
        ..
    } = &mut statement
    {
        loop {
            let next = parser.peek_token();
            if next.span.start.line <= parser.get_current_token().span.end.line {
                break;
            }
            let Token::SingleQuotedString(fragment) = next.token else {
                break;
            };
            parser.next_token();
            comment.push_str(&fragment);
        }
    }
    Ok(statement)
}

fn skip_signature(parser: &mut Parser<'_>) -> Result<(), ParserError> {
    let mut depth = 1;
    while depth > 0 {
        match parser.peek_token().token {
            Token::EOF | Token::SemiColon => {
                return Err(ParserError::ParserError(
                    "Unterminated COMMENT ON routine signature".into(),
                ));
            }
            Token::LParen => depth += 1,
            Token::RParen => depth -= 1,
            _ => {}
        }
        parser.next_token();
    }
    Ok(())
}
