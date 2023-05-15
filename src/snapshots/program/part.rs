
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    pub wo: String,
    pub qty: u32
}
