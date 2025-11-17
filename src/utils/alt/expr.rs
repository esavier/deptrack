use super::evaluable::Evaluable;

#[derive(Debug, Clone)]
pub enum LogicExpr<T> {
    And(Box<LogicExpr<T>>, Box<LogicExpr<T>>),
    Or(Box<LogicExpr<T>>, Box<LogicExpr<T>>),
    Xor(Box<LogicExpr<T>>, Box<LogicExpr<T>>),
    Not(Box<LogicExpr<T>>),
    Leaf(T),
}

pub struct ExprBuilder<T> {
    expr: LogicExpr<T>,
}

impl<T> ExprBuilder<T> {
    pub fn new(predicate: T) -> Self {
        Self {
            expr: LogicExpr::Leaf(predicate),
        }
    }

    pub fn and(self, other: impl Into<LogicExpr<T>>) -> Self {
        Self {
            expr: LogicExpr::And(Box::new(self.expr), Box::new(other.into())),
        }
    }

    pub fn or(self, other: impl Into<LogicExpr<T>>) -> Self {
        Self {
            expr: LogicExpr::Or(Box::new(self.expr), Box::new(other.into())),
        }
    }

    pub fn xor(self, other: impl Into<LogicExpr<T>>) -> Self {
        Self {
            expr: LogicExpr::Xor(Box::new(self.expr), Box::new(other.into())),
        }
    }

    pub fn negate(self) -> Self {
        Self {
            expr: LogicExpr::Not(Box::new(self.expr)),
        }
    }

    pub fn build(self) -> LogicExpr<T> {
        self.expr
    }
}

impl<T> LogicExpr<T> {
    pub fn and(left: LogicExpr<T>, right: LogicExpr<T>) -> Self {
        LogicExpr::And(Box::new(left), Box::new(right))
    }

    pub fn or(left: LogicExpr<T>, right: LogicExpr<T>) -> Self {
        LogicExpr::Or(Box::new(left), Box::new(right))
    }

    pub fn xor(left: LogicExpr<T>, right: LogicExpr<T>) -> Self {
        LogicExpr::Xor(Box::new(left), Box::new(right))
    }

    pub fn negate(expr: LogicExpr<T>) -> Self {
        LogicExpr::Not(Box::new(expr))
    }

    pub fn leaf(predicate: T) -> Self {
        LogicExpr::Leaf(predicate)
    }
}

impl<T> LogicExpr<T>
where
    T: Evaluable,
{
    pub fn evaluate(&self, context: &T::Context) -> Result<bool, T::Error> {
        match self {
            LogicExpr::And(left, right) => {
                let left_result = left.evaluate(context)?;
                if !left_result {
                    return Ok(false);
                }
                right.evaluate(context)
            }
            LogicExpr::Or(left, right) => {
                let left_result = left.evaluate(context)?;
                if left_result {
                    return Ok(true);
                }
                right.evaluate(context)
            }
            LogicExpr::Xor(left, right) => {
                let left_result = left.evaluate(context)?;
                let right_result = right.evaluate(context)?;
                Ok(left_result != right_result)
            }
            LogicExpr::Not(expr) => {
                let result = expr.evaluate(context)?;
                Ok(!result)
            }
            LogicExpr::Leaf(predicate) => predicate.evaluate(context),
        }
    }
}

impl<T> From<T> for LogicExpr<T> {
    fn from(predicate: T) -> Self {
        LogicExpr::Leaf(predicate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestPredicate {
        result: bool,
        #[allow(dead_code)]
        name: String,
    }

    impl TestPredicate {
        fn new(name: &str, result: bool) -> Self {
            Self {
                result,
                name: name.to_string(),
            }
        }
    }

    impl Evaluable for TestPredicate {
        type Context = ();
        type Error = &'static str;

        fn evaluate(&self, _context: &Self::Context) -> Result<bool, Self::Error> {
            Ok(self.result)
        }
    }

    #[test]
    fn test_leaf_evaluation() {
        let predicate = TestPredicate::new("true_pred", true);
        let expr = LogicExpr::Leaf(predicate);

        assert!(expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_and_evaluation() {
        let true_pred = TestPredicate::new("true", true);
        let false_pred = TestPredicate::new("false", false);

        let expr = LogicExpr::and(
            LogicExpr::Leaf(true_pred.clone()),
            LogicExpr::Leaf(true_pred.clone()),
        );
        assert!(expr.evaluate(&()).unwrap());

        let expr = LogicExpr::and(LogicExpr::Leaf(true_pred), LogicExpr::Leaf(false_pred));
        assert!(!expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_or_evaluation() {
        let true_pred = TestPredicate::new("true", true);
        let false_pred = TestPredicate::new("false", false);

        let expr = LogicExpr::or(
            LogicExpr::Leaf(false_pred.clone()),
            LogicExpr::Leaf(true_pred),
        );
        assert!(expr.evaluate(&()).unwrap());

        let expr = LogicExpr::or(
            LogicExpr::Leaf(false_pred.clone()),
            LogicExpr::Leaf(false_pred),
        );
        assert!(!expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_xor_evaluation() {
        let true_pred = TestPredicate::new("true", true);
        let false_pred = TestPredicate::new("false", false);

        let expr = LogicExpr::xor(
            LogicExpr::Leaf(true_pred.clone()),
            LogicExpr::Leaf(false_pred.clone()),
        );
        assert!(expr.evaluate(&()).unwrap());

        let expr = LogicExpr::xor(
            LogicExpr::Leaf(true_pred.clone()),
            LogicExpr::Leaf(true_pred),
        );
        assert!(!expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_not_evaluation() {
        let true_pred = TestPredicate::new("true", true);
        let false_pred = TestPredicate::new("false", false);

        let expr = LogicExpr::negate(LogicExpr::Leaf(true_pred));
        assert!(!expr.evaluate(&()).unwrap());

        let expr = LogicExpr::negate(LogicExpr::Leaf(false_pred));
        assert!(expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_complex_expression() {
        let true_pred = TestPredicate::new("true", true);
        let false_pred = TestPredicate::new("false", false);

        // (true AND false) OR (NOT false)
        let expr = LogicExpr::or(
            LogicExpr::and(
                LogicExpr::Leaf(true_pred),
                LogicExpr::Leaf(false_pred.clone()),
            ),
            LogicExpr::negate(LogicExpr::Leaf(false_pred)),
        );

        assert!(expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_short_circuit_and() {
        let false_pred = TestPredicate::new("false", false);
        let true_pred = TestPredicate::new("true", true);

        // false AND true should short-circuit and not evaluate the second predicate
        let expr = LogicExpr::and(LogicExpr::Leaf(false_pred), LogicExpr::Leaf(true_pred));

        assert!(!expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_short_circuit_or() {
        let true_pred = TestPredicate::new("true", true);
        let false_pred = TestPredicate::new("false", false);

        // true OR false should short-circuit and not evaluate the second predicate
        let expr = LogicExpr::or(LogicExpr::Leaf(true_pred), LogicExpr::Leaf(false_pred));

        assert!(expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_builder_pattern() {
        let true_pred = TestPredicate::new("true", true);
        let false_pred = TestPredicate::new("false", false);

        let expr = ExprBuilder::new(true_pred.clone())
            .and(false_pred.clone())
            .or(true_pred)
            .build();

        // (true AND false) OR true = true
        assert!(expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_builder_not() {
        let false_pred = TestPredicate::new("false", false);

        let expr = ExprBuilder::new(false_pred).negate().build();

        assert!(expr.evaluate(&()).unwrap());
    }

    #[test]
    fn test_from_conversion() {
        let pred = TestPredicate::new("test", true);
        let expr: LogicExpr<TestPredicate> = pred.into();

        assert!(expr.evaluate(&()).unwrap());
    }
}
