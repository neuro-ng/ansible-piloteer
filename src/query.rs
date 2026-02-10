use jmespath::functions::Function;
use jmespath::interpret; // Use root re-export
use jmespath::{Context, ErrorReason, JmespathError, Rcvar, Variable};
use std::collections::BTreeMap;
use std::rc::Rc;

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
