use crate::types::Value;

/// Trait for extensible aggregation functions.
/// Implement this to add new aggregate operations (e.g., STDDEV, MEDIAN).
pub trait Aggregator: std::fmt::Debug {
    /// Feed a value into the aggregation.
    fn accumulate(&mut self, value: &Value);

    /// Produce the final result.
    fn finish(&self) -> Value;

    /// Create a fresh instance of this aggregator (for use in multiple groups).
    fn clone_box(&self) -> Box<dyn Aggregator>;
}

// ── COUNT ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CountAgg {
    count: i64,
    count_star: bool,
}

impl CountAgg {
    pub fn new(count_star: bool) -> Self {
        CountAgg {
            count: 0,
            count_star,
        }
    }
}

impl Aggregator for CountAgg {
    fn accumulate(&mut self, value: &Value) {
        if self.count_star || !value.is_null() {
            self.count += 1;
        }
    }

    fn finish(&self) -> Value {
        Value::Integer(self.count)
    }

    fn clone_box(&self) -> Box<dyn Aggregator> {
        Box::new(self.clone())
    }
}

// ── SUM ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SumAgg {
    sum: f64,
    has_value: bool,
    all_integers: bool,
}

impl SumAgg {
    pub fn new() -> Self {
        SumAgg {
            sum: 0.0,
            has_value: false,
            all_integers: true,
        }
    }
}

impl Aggregator for SumAgg {
    fn accumulate(&mut self, value: &Value) {
        if let Some(n) = value.to_f64() {
            self.sum += n;
            self.has_value = true;
            if !matches!(value, Value::Integer(_)) {
                self.all_integers = false;
            }
        }
    }

    fn finish(&self) -> Value {
        if !self.has_value {
            return Value::Null;
        }
        if self.all_integers {
            Value::Integer(self.sum as i64)
        } else {
            Value::Float(self.sum)
        }
    }

    fn clone_box(&self) -> Box<dyn Aggregator> {
        Box::new(self.clone())
    }
}

// ── AVG ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AvgAgg {
    sum: f64,
    count: i64,
}

impl AvgAgg {
    pub fn new() -> Self {
        AvgAgg { sum: 0.0, count: 0 }
    }
}

impl Aggregator for AvgAgg {
    fn accumulate(&mut self, value: &Value) {
        if let Some(n) = value.to_f64() {
            self.sum += n;
            self.count += 1;
        }
    }

    fn finish(&self) -> Value {
        if self.count == 0 {
            Value::Null
        } else {
            Value::Float(self.sum / self.count as f64)
        }
    }

    fn clone_box(&self) -> Box<dyn Aggregator> {
        Box::new(self.clone())
    }
}

// ── MIN ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MinAgg {
    min: Option<Value>,
}

impl MinAgg {
    pub fn new() -> Self {
        MinAgg { min: None }
    }
}

impl Aggregator for MinAgg {
    fn accumulate(&mut self, value: &Value) {
        if value.is_null() {
            return;
        }
        self.min = Some(match &self.min {
            None => value.clone(),
            Some(current) => {
                if value < current {
                    value.clone()
                } else {
                    current.clone()
                }
            }
        });
    }

    fn finish(&self) -> Value {
        self.min.clone().unwrap_or(Value::Null)
    }

    fn clone_box(&self) -> Box<dyn Aggregator> {
        Box::new(self.clone())
    }
}

// ── MAX ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MaxAgg {
    max: Option<Value>,
}

impl MaxAgg {
    pub fn new() -> Self {
        MaxAgg { max: None }
    }
}

impl Aggregator for MaxAgg {
    fn accumulate(&mut self, value: &Value) {
        if value.is_null() {
            return;
        }
        self.max = Some(match &self.max {
            None => value.clone(),
            Some(current) => {
                if value > current {
                    value.clone()
                } else {
                    current.clone()
                }
            }
        });
    }

    fn finish(&self) -> Value {
        self.max.clone().unwrap_or(Value::Null)
    }

    fn clone_box(&self) -> Box<dyn Aggregator> {
        Box::new(self.clone())
    }
}

/// Create an aggregator by function name.
pub fn create_aggregator(name: &str, is_star: bool) -> Option<Box<dyn Aggregator>> {
    match name.to_uppercase().as_str() {
        "COUNT" => Some(Box::new(CountAgg::new(is_star))),
        "SUM" => Some(Box::new(SumAgg::new())),
        "AVG" => Some(Box::new(AvgAgg::new())),
        "MIN" => Some(Box::new(MinAgg::new())),
        "MAX" => Some(Box::new(MaxAgg::new())),
        _ => None,
    }
}
