
use std::hash::{Hash, Hasher};

#[derive(Debug, Default, Clone, Eq, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    pub wo: String,
    pub qty: u32
}

impl Part {
    /// Tests equality of not only the part but the quantity as well
    /// 
    /// using `self == other` only test equality of `name` and `wo`
    /// ```
    /// # use db_mutations_tracker::snapshots::program::Part;
    /// let a = Part { name: String::from("x1a"), wo: String::from("something"), qty: 1 };
    /// let b = Part { name: String::from("x1a"), wo: String::from("something"), qty: 5 };
    /// let c = Part { name: String::from("x1a"), wo: String::from("something"), qty: 1 };
    /// assert!(a.equals(&c));
    /// assert!(!a.equals(&b));
    /// ```
    pub fn equals(&self, other: &Self) -> bool {
        self == other && self.qty == other.qty
    }
}

// TODO: make this TryFrom and return entity that failed
impl From<tiberius::Row> for Part {
    fn from(row: tiberius::Row) -> Self {
        Part {
            name: row.get::<&str, _>("name").unwrap().into(),
            wo:   row.get::<&str, _>("workorder").unwrap().into(),
            qty:  row.get::<i32,  _>("qty" ).unwrap().unsigned_abs(),
        }
    }
}

impl Hash for Part {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.wo.hash(state);
    }
}

impl PartialEq<Part> for Part {
    /// test equality based on `name` and `wo`
    /// 
    /// ```
    /// # use db_mutations_tracker::snapshots::program::Part;
    /// 
    /// let a = Part { name: String::from("x1a"), wo: String::from("something"), qty: 1 };
    /// let b = Part { name: String::from("x1a"), wo: String::from("something"), qty: 5 };
    /// assert_eq!(a, b);
    /// ```
    fn eq(&self, other: &Part) -> bool {
        self.name == other.name && self.wo == other.wo
    }
}
