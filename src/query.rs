use jmespath::functions::Function;
use jmespath::interpret; // Use root re-export
use jmespath::{Context, ErrorReason, JmespathError, Rcvar, Runtime, Variable};
use std::collections::BTreeMap;
use std::rc::Rc;

pub fn register_functions(runtime: &mut Runtime) {
    runtime.register_function("group_by", Box::new(GroupBy::new()));
    runtime.register_function("unique", Box::new(Unique::new()));
    runtime.register_function("count", Box::new(Count::new()));
    runtime.register_function("sum", Box::new(Sum::new()));
    runtime.register_function("avg", Box::new(Avg::new()));
    runtime.register_function("min", Box::new(Min::new()));
    runtime.register_function("max", Box::new(Max::new()));
    // New utility functions
    runtime.register_function("replace", Box::new(Replace::new()));
    runtime.register_function("split", Box::new(Split::new()));
    runtime.register_function("matches", Box::new(Matches::new()));
}

pub struct GroupBy;

impl Default for GroupBy {
    fn default() -> Self {
        Self
    }
}

impl GroupBy {
    pub fn new() -> Self {
        Self
    }
}

impl Function for GroupBy {
    fn evaluate(&self, args: &[Rcvar], ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        let items = args[0].as_array().ok_or_else(|| {
            JmespathError::new(
                "group_by",
                0,
                ErrorReason::Parse("First argument must be an array".to_string()),
            )
        })?;

        let expression = match &*args[1] {
            Variable::Expref(ast) => ast,
            _ => {
                return Err(JmespathError::new(
                    "group_by",
                    0,
                    ErrorReason::Parse("Second argument must be an expression".to_string()),
                ));
            }
        };

        let mut groups: BTreeMap<String, Vec<Rcvar>> = BTreeMap::new();

        for item in items {
            let key_var = interpret(item, expression, ctx)?;

            // Convert key to string representation
            // If it's a string, use it directly. Otherwise JSON stringify.
            let key_str = if let Variable::String(s) = &*key_var {
                s.clone()
            } else {
                serde_json::to_string(&*key_var).unwrap_or_else(|_| "null".to_string())
            };

            groups.entry(key_str).or_default().push(item.clone());
        }

        let mut result_map = BTreeMap::new();
        for (k, v) in groups {
            result_map.insert(k, Rc::new(Variable::Array(v)));
        }

        Ok(Rc::new(Variable::Object(result_map)))
    }
}

pub struct Unique;

impl Default for Unique {
    fn default() -> Self {
        Self
    }
}

impl Unique {
    pub fn new() -> Self {
        Self
    }
}

impl Function for Unique {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        let items = args[0].as_array().ok_or_else(|| {
            JmespathError::new(
                "unique",
                0,
                ErrorReason::Parse("First argument must be an array".to_string()),
            )
        })?;

        let mut unique_items = Vec::new();

        // Use PartialEq for uniqueness
        for item in items {
            if !unique_items.contains(item) {
                unique_items.push(item.clone());
            }
        }

        Ok(Rc::new(Variable::Array(unique_items)))
    }
}

// Aggregation Functions

pub struct Count;

impl Default for Count {
    fn default() -> Self {
        Self
    }
}

impl Count {
    pub fn new() -> Self {
        Self
    }
}

impl Function for Count {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        let items = args[0].as_array().ok_or_else(|| {
            JmespathError::new(
                "count",
                0,
                ErrorReason::Parse("Argument must be an array".to_string()),
            )
        })?;

        let count = serde_json::Number::from_f64(items.len() as f64).unwrap();
        Ok(Rc::new(Variable::Number(count)))
    }
}

pub struct Sum;

impl Default for Sum {
    fn default() -> Self {
        Self
    }
}

impl Sum {
    pub fn new() -> Self {
        Self
    }
}

impl Function for Sum {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        let items = args[0].as_array().ok_or_else(|| {
            JmespathError::new(
                "sum",
                0,
                ErrorReason::Parse("Argument must be an array".to_string()),
            )
        })?;

        let mut total = 0.0;
        for item in items {
            if let Variable::Number(n) = &**item
                && let Some(f) = n.as_f64()
            {
                total += f;
            }
        }

        let result = serde_json::Number::from_f64(total).unwrap();
        Ok(Rc::new(Variable::Number(result)))
    }
}

pub struct Avg;

impl Default for Avg {
    fn default() -> Self {
        Self
    }
}

impl Avg {
    pub fn new() -> Self {
        Self
    }
}

impl Function for Avg {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        let items = args[0].as_array().ok_or_else(|| {
            JmespathError::new(
                "avg",
                0,
                ErrorReason::Parse("Argument must be an array".to_string()),
            )
        })?;

        if items.is_empty() {
            return Ok(Rc::new(Variable::Null));
        }

        let mut total = 0.0;
        let mut count = 0;
        for item in items {
            if let Variable::Number(n) = &**item
                && let Some(f) = n.as_f64()
            {
                total += f;
                count += 1;
            }
        }

        if count == 0 {
            Ok(Rc::new(Variable::Null))
        } else {
            let avg = serde_json::Number::from_f64(total / count as f64).unwrap();
            Ok(Rc::new(Variable::Number(avg)))
        }
    }
}

pub struct Min;

impl Default for Min {
    fn default() -> Self {
        Self
    }
}

impl Min {
    pub fn new() -> Self {
        Self
    }
}

impl Function for Min {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        let items = args[0].as_array().ok_or_else(|| {
            JmespathError::new(
                "min",
                0,
                ErrorReason::Parse("Argument must be an array".to_string()),
            )
        })?;

        let mut min_val: Option<f64> = None;
        for item in items {
            if let Variable::Number(n) = &**item
                && let Some(f) = n.as_f64()
            {
                min_val = Some(min_val.map_or(f, |m| m.min(f)));
            }
        }

        Ok(Rc::new(min_val.map_or(Variable::Null, |v| {
            Variable::Number(serde_json::Number::from_f64(v).unwrap())
        })))
    }
}

pub struct Max;

impl Default for Max {
    fn default() -> Self {
        Self
    }
}

impl Max {
    pub fn new() -> Self {
        Self
    }
}

impl Function for Max {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        let items = args[0].as_array().ok_or_else(|| {
            JmespathError::new(
                "max",
                0,
                ErrorReason::Parse("Argument must be an array".to_string()),
            )
        })?;

        let mut max_val: Option<f64> = None;
        for item in items {
            if let Variable::Number(n) = &**item
                && let Some(f) = n.as_f64()
            {
                max_val = Some(max_val.map_or(f, |m| m.max(f)));
            }
        }

        Ok(Rc::new(max_val.map_or(Variable::Null, |v| {
            Variable::Number(serde_json::Number::from_f64(v).unwrap())
        })))
    }
}

// Custom filter implementation
// Redefine to hold String
pub struct CustomFilter {
    expression_str: String,
    // We create a new local runtime for each filter execution to ensure isolation
    // but we Register user functions into it.
}

impl CustomFilter {
    pub fn new(expression_str: String) -> Self {
        Self { expression_str }
    }
}

impl Function for CustomFilter {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        // We only support 1 argument (the current node) which is passed as `@` context implicitely
        // or explicitly via arguments. But JMESPath filters usually take the subject as input.
        // Wait, standard JMESPath functions take arguments.
        // Our syntax `my_filter(@)` passes current node as argument.
        // So we expect at least 1 argument.

        // We create a fresh runtime to evaluate the inner expression.
        // IMPORTANT: We must register the SAME custom functions (except recursive custom filters)
        // so that the inner expression can use `count()`, `group_by()`, etc.
        let mut runtime = Runtime::new();
        runtime.register_builtin_functions();
        register_functions(&mut runtime);

        let expr = runtime.compile(&self.expression_str)?;

        // If the user called `my_filter(some_array)`, `args[0]` is `some_array`.
        // The expression `count(@)` will be evaluated against `args[0]`.
        let subject = if !args.is_empty() {
            args[0].clone()
        } else {
            // Should not happen if called as a function with args
            return Err(JmespathError::new(
                "custom_filter",
                0,
                ErrorReason::Parse("Custom filter expects at least 1 argument".to_string()),
            ));
        };

        let result = expr.search(&subject)?;
        Ok(result)
    }
}

// Utility Functions

pub struct Replace;
impl Replace {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Replace {
    fn default() -> Self {
        Self::new()
    }
}

impl Function for Replace {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        if args.len() != 3 {
            return Err(JmespathError::new(
                "replace",
                0,
                ErrorReason::Parse(
                    "replace() expects 3 arguments: string, pattern, replacement".to_string(),
                ),
            ));
        }
        let input = args[0].as_string().ok_or_else(|| {
            JmespathError::new(
                "replace",
                0,
                ErrorReason::Parse("First argument must be a string".to_string()),
            )
        })?;
        let pattern = args[1].as_string().ok_or_else(|| {
            JmespathError::new(
                "replace",
                0,
                ErrorReason::Parse("Second argument must be a string".to_string()),
            )
        })?;
        let replacement = args[2].as_string().ok_or_else(|| {
            JmespathError::new(
                "replace",
                0,
                ErrorReason::Parse("Third argument must be a string".to_string()),
            )
        })?;

        let result = input.replace(pattern, replacement);
        Ok(Rc::new(Variable::String(result)))
    }
}

pub struct Split;
impl Split {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Split {
    fn default() -> Self {
        Self::new()
    }
}

impl Function for Split {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        if args.len() != 2 {
            return Err(JmespathError::new(
                "split",
                0,
                ErrorReason::Parse("split() expects 2 arguments: string, separator".to_string()),
            ));
        }
        let input = args[0].as_string().ok_or_else(|| {
            JmespathError::new(
                "split",
                0,
                ErrorReason::Parse("First argument must be a string".to_string()),
            )
        })?;
        let separator = args[1].as_string().ok_or_else(|| {
            JmespathError::new(
                "split",
                0,
                ErrorReason::Parse("Second argument must be a string".to_string()),
            )
        })?;

        let parts: Vec<Rcvar> = input
            .split(separator)
            .map(|s| Rc::new(Variable::String(s.to_string())))
            .collect();
        Ok(Rc::new(Variable::Array(parts)))
    }
}

pub struct Matches;
impl Matches {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Matches {
    fn default() -> Self {
        Self::new()
    }
}

impl Function for Matches {
    fn evaluate(&self, args: &[Rcvar], _ctx: &mut Context<'_>) -> Result<Rcvar, JmespathError> {
        if args.len() != 2 {
            return Err(JmespathError::new(
                "matches",
                0,
                ErrorReason::Parse(
                    "matches() expects 2 arguments: string, regex_pattern".to_string(),
                ),
            ));
        }
        let input = args[0].as_string().ok_or_else(|| {
            JmespathError::new(
                "matches",
                0,
                ErrorReason::Parse("First argument must be a string".to_string()),
            )
        })?;
        let pattern = args[1].as_string().ok_or_else(|| {
            JmespathError::new(
                "matches",
                0,
                ErrorReason::Parse("Second argument must be a string".to_string()),
            )
        })?;

        let re = regex::Regex::new(pattern).map_err(|e| {
            JmespathError::new(
                "matches",
                0,
                ErrorReason::Parse(format!("Invalid regex: {}", e)),
            )
        })?;

        Ok(Rc::new(Variable::Bool(re.is_match(input))))
    }
}
