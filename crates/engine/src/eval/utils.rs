use eyre::{bail, Result};
use solang_parser::{
    parse,
    pt::{Expression, Identifier, SourceUnit, SourceUnitPart, Statement},
};

pub fn parse_input(input: &str) -> Result<Expression> {
    let trimmed = input.trim();
    let wrapped_input = if trimmed.ends_with(";") {
        format!("function __edb_sol_repl_() public {{ {} }}", trimmed)
    } else {
        format!("function __edb_sol_repl_() public {{ {}; }}", trimmed)
    };

    let (SourceUnit(parts), _comments) =
        parse(&wrapped_input, 0).map_err(|e| eyre::eyre!("Parse error: {:?}", e))?;

    if parts.len() != 1 {
        bail!("Expected a single function definition");
    }
    let statements = match parts[0] {
        SourceUnitPart::FunctionDefinition(ref def) => {
            if let Some(Statement::Block { ref statements, .. }) = def.body {
                statements.clone()
            } else {
                bail!("Function body is not a block");
            }
        }
        _ => bail!("Expected a function definition"),
    };

    if statements.len() != 1 {
        bail!("Expected a single statement in function body");
    }
    if let Statement::Expression(_, ref expr) = statements[0] {
        Ok(expr.clone())
    } else {
        bail!("Expected an expression statement")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_input() {
        let expr = parse_input("a + b").unwrap();
        if let Expression::Add(_, left, right) = expr {
            if let (
                Expression::Variable(Identifier { name: n1, .. }),
                Expression::Variable(Identifier { name: n2, .. }),
            ) = (*left, *right)
            {
                assert_eq!(n1, "a");
                assert_eq!(n2, "b");
            } else {
                panic!("Add operands are not variables");
            }
        } else {
            panic!("Parsed expression is not an addition");
        }

        let expr = parse_input("x * (y - z);").unwrap();
        if let Expression::Multiply(_, left, right) = expr {
            if let Expression::Variable(Identifier { name: n1, .. }) = *left {
                // The parenthesis might result in a Parenthesis expression wrapping the subtraction
                match *right {
                    Expression::Subtract(_, sub_left, sub_right) => {
                        if let (
                            Expression::Variable(Identifier { name: n2, .. }),
                            Expression::Variable(Identifier { name: n3, .. }),
                        ) = (*sub_left, *sub_right)
                        {
                            assert_eq!(n1, "x");
                            assert_eq!(n2, "y");
                            assert_eq!(n3, "z");
                        } else {
                            panic!("Subtraction operands are not variables");
                        }
                    }
                    Expression::Parenthesis(_, inner) => {
                        if let Expression::Subtract(_, sub_left, sub_right) = *inner {
                            if let (
                                Expression::Variable(Identifier { name: n2, .. }),
                                Expression::Variable(Identifier { name: n3, .. }),
                            ) = (*sub_left, *sub_right)
                            {
                                assert_eq!(n1, "x");
                                assert_eq!(n2, "y");
                                assert_eq!(n3, "z");
                            } else {
                                panic!("Subtraction operands are not variables");
                            }
                        } else {
                            panic!("Parenthesis does not contain a subtraction");
                        }
                    }
                    _ => {
                        panic!("Right operand is neither subtraction nor parenthesis: {:?}", right)
                    }
                }
            } else {
                panic!("Left operand is not a variable");
            }
        } else {
            panic!("Parsed expression is not a multiplication");
        }
    }
}
