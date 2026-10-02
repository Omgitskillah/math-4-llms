use super::{ErrorCode as Code, MathError};

#[derive(Clone, Copy, PartialEq)]
enum Token {
    Number,
    Function,
    Constant,
    Plus,
    Minus,
    Multiply,
    Divide,
    Power,
    Open,
    Close,
}

fn error(code: Code, position: usize) -> MathError {
    MathError::new(code, Some(position))
}

// Decimal/scientific literals only; signs belong to the grammar, not the lexer.
fn number_end(bytes: &[u8], mut i: usize) -> Option<usize> {
    let mut digits = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        digits += 1;
        i += 1;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            digits += 1;
            i += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    Some(i)
}

pub(super) fn is_scalar(text: &str) -> bool {
    let text = text.strip_prefix('-').unwrap_or(text);
    number_end(text.as_bytes(), 0) == Some(text.len()) && !text.is_empty()
}

pub(super) fn validate(input: &str) -> Result<(), MathError> {
    if input.len() > 1024 {
        return Err(MathError::new(Code::InputTooLong, None));
    }
    if input.trim().is_empty() {
        return Err(MathError::new(Code::EmptyInput, None));
    }
    let mut tokens = Vec::new();
    let mut opens = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let ch = input[i..].chars().next().expect("character boundary");
        if ch.is_whitespace() {
            i += ch.len_utf8();
            continue;
        }
        let start = i;
        let token = match ch {
            '`' | '$' | '\\' | '"' | '\'' | '[' | ']' | '{' | '}' => {
                return Err(error(Code::UnsupportedFormatting, i));
            }
            ',' => return Err(error(Code::AmbiguousComma, i)),
            ';' => return Err(error(Code::MultipleStatements, i)),
            '=' => return Err(error(Code::AssignmentNotAllowed, i)),
            '(' => {
                opens.push(i);
                if opens.len() > 32 {
                    return Err(error(Code::NestingTooDeep, i));
                }
                i += 1;
                Token::Open
            }
            ')' => {
                if opens.pop().is_none() {
                    return Err(error(Code::UnbalancedParentheses, i));
                }
                i += 1;
                Token::Close
            }
            '+' => {
                i += 1;
                Token::Plus
            }
            '-' => {
                i += 1;
                Token::Minus
            }
            '*' => {
                i += 1;
                Token::Multiply
            }
            '/' => {
                i += 1;
                Token::Divide
            }
            '^' => {
                i += 1;
                Token::Power
            }
            '0'..='9' | '.' => {
                i = number_end(input.as_bytes(), i).ok_or_else(|| error(Code::InvalidSyntax, i))?;
                Token::Number
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                while i < input.len()
                    && (input.as_bytes()[i].is_ascii_alphanumeric() || input.as_bytes()[i] == b'_')
                {
                    i += 1;
                }
                match &input[start..i] {
                    "sqrt" | "exp" | "ln" | "log10" | "sin" | "cos" | "tan" | "abs" => {
                        Token::Function
                    }
                    "pi" | "e" => Token::Constant,
                    _ => return Err(error(Code::UnknownIdentifier, start)),
                }
            }
            _ => return Err(error(Code::UnsupportedCharacter, i)),
        };
        tokens.push((token, start));
    }
    if let Some(position) = opens.last() {
        return Err(error(Code::UnbalancedParentheses, *position));
    }
    let mut parser = Parser {
        tokens: &tokens,
        index: 0,
        end: input.len(),
    };
    parser.expression()?;
    if parser.index != tokens.len() {
        return Err(parser.invalid());
    }
    Ok(())
}

struct Parser<'a> {
    tokens: &'a [(Token, usize)],
    index: usize,
    end: usize,
}
impl Parser<'_> {
    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.index).map(|x| x.0)
    }
    fn invalid(&self) -> MathError {
        error(
            Code::InvalidSyntax,
            self.tokens.get(self.index).map_or(self.end, |x| x.1),
        )
    }
    // Validate operand/operator sequence. Fend retains precedence/evaluation.
    fn expression(&mut self) -> Result<(), MathError> {
        self.operand()?;
        while matches!(
            self.peek(),
            Some(Token::Plus | Token::Minus | Token::Multiply | Token::Divide | Token::Power)
        ) {
            self.index += 1;
            self.operand()?;
        }
        Ok(())
    }
    fn operand(&mut self) -> Result<(), MathError> {
        // One unary sign is allowed; repeated signs are never silently repaired.
        if matches!(self.peek(), Some(Token::Plus | Token::Minus)) {
            self.index += 1;
        }
        match self.peek() {
            Some(Token::Number | Token::Constant) => {
                self.index += 1;
                Ok(())
            }
            Some(Token::Function) => {
                self.index += 1;
                if self.peek() != Some(Token::Open) {
                    return Err(self.invalid());
                }
                self.group()
            }
            Some(Token::Open) => self.group(),
            _ => Err(self.invalid()),
        }
    }
    fn group(&mut self) -> Result<(), MathError> {
        self.index += 1;
        self.expression()?;
        if self.peek() != Some(Token::Close) {
            return Err(self.invalid());
        }
        self.index += 1;
        Ok(())
    }
}
