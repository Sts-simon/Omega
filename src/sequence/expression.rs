use std::collections::BTreeMap;

pub fn evaluate(expression: &str, variables: &BTreeMap<String, f64>) -> Result<f64, String> {
    if expression.trim().is_empty() || expression.len() > 128 {
        return Err("Expression must contain 1 to 128 characters".to_string());
    }
    let mut parser = Parser {
        chars: expression.chars().collect(),
        index: 0,
        variables,
    };
    let value = parser.parse_expression()?;
    parser.skip_whitespace();
    if parser.index != parser.chars.len() {
        return Err(format!("Unexpected character at position {}", parser.index + 1));
    }
    if !value.is_finite() {
        return Err("Expression result is not finite".to_string());
    }
    Ok(value)
}

struct Parser<'a> {
    chars: Vec<char>,
    index: usize,
    variables: &'a BTreeMap<String, f64>,
}

impl Parser<'_> {
    fn parse_expression(&mut self) -> Result<f64, String> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_whitespace();
            if self.consume('+') {
                value += self.parse_term()?;
            } else if self.consume('-') {
                value -= self.parse_term()?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        let mut value = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            if self.consume('*') {
                value *= self.parse_unary()?;
            } else if self.consume('/') {
                let divisor = self.parse_unary()?;
                if divisor.abs() <= f64::EPSILON {
                    return Err("Division by zero".to_string());
                }
                value /= divisor;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<f64, String> {
        self.skip_whitespace();
        if self.consume('+') {
            self.parse_unary()
        } else if self.consume('-') {
            Ok(-self.parse_unary()?)
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<f64, String> {
        self.skip_whitespace();
        if self.consume('(') {
            let value = self.parse_expression()?;
            self.skip_whitespace();
            if !self.consume(')') {
                return Err("Missing closing parenthesis".to_string());
            }
            return Ok(value);
        }

        if self
            .peek()
            .is_some_and(|character| character.is_ascii_digit() || character == '.')
        {
            return self.parse_number();
        }
        if self
            .peek()
            .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        {
            let name = self.parse_identifier();
            return self
                .variables
                .get(&name)
                .copied()
                .ok_or_else(|| format!("Undefined variable '{name}'"));
        }
        Err(format!(
            "Expected a number, variable or '(' at position {}",
            self.index + 1
        ))
    }

    fn parse_number(&mut self) -> Result<f64, String> {
        let start = self.index;
        let mut has_digit = false;
        while self.peek().is_some_and(|character| character.is_ascii_digit()) {
            has_digit = true;
            self.index += 1;
        }
        if self.consume('.') {
            while self.peek().is_some_and(|character| character.is_ascii_digit()) {
                has_digit = true;
                self.index += 1;
            }
        }
        if self
            .peek()
            .is_some_and(|character| character == 'e' || character == 'E')
        {
            self.index += 1;
            if self
                .peek()
                .is_some_and(|character| character == '+' || character == '-')
            {
                self.index += 1;
            }
            let exponent_start = self.index;
            while self.peek().is_some_and(|character| character.is_ascii_digit()) {
                self.index += 1;
            }
            if exponent_start == self.index {
                return Err("Malformed exponent".to_string());
            }
        }
        if !has_digit {
            return Err("Malformed numeric literal".to_string());
        }
        let token: String = self.chars[start..self.index].iter().collect();
        token.parse::<f64>().map_err(|_| format!("Invalid number '{token}'"))
    }

    fn parse_identifier(&mut self) -> String {
        let start = self.index;
        self.index += 1;
        while self
            .peek()
            .is_some_and(|character| character == '_' || character.is_ascii_alphanumeric())
        {
            self.index += 1;
        }
        self.chars[start..self.index].iter().collect()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|character| character.is_ascii_whitespace()) {
            self.index += 1;
        }
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_operator_precedence_and_parentheses() {
        let variables = BTreeMap::from([("offset".to_string(), 2.0)]);
        assert_eq!(evaluate("(3 + offset) * 4 / 2", &variables).unwrap(), 10.0);
    }

    #[test]
    fn supports_unary_and_exponent_values() {
        let variables = BTreeMap::new();
        assert_eq!(evaluate("-2.5e1 + +5", &variables).unwrap(), -20.0);
    }

    #[test]
    fn rejects_undefined_variables_and_division_by_zero() {
        let variables = BTreeMap::new();
        assert!(evaluate("missing + 1", &variables).is_err());
        assert!(evaluate("4 / (2 - 2)", &variables).is_err());
    }
}
