
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    pub grade: String,
    pub mm: String,
    pub heat: String,
    pub po: u64
}

impl Sheet {
    pub fn update(&mut self, data: &Vec<SheetData>) {
        for data_point in data {
            match data_point {
                // name change should not happen because that is a different sheet
                SheetData::Grade(grade)       => self.grade = grade.to_string(),
                SheetData::MaterialMaster(mm) => self.mm    = mm.to_string(),
                SheetData::HeatNumber(heat)   => self.heat  = heat.to_string(),
                SheetData::PoNumber(po)       => self.po    = *po,
            }
        }
    }

    pub fn diff_sheet(&self, other: Self) -> Vec<SheetData> {
        let mut sheet_diffs = Vec::new();

        if self.grade != other.grade
            { sheet_diffs.push(SheetData::Grade(other.grade)) }
        if self.mm != other.mm
            { sheet_diffs.push(SheetData::MaterialMaster(other.mm)) }
        if self.heat != other.heat
            { sheet_diffs.push(SheetData::HeatNumber(other.heat)) }
        if self.po != other.po
            { sheet_diffs.push(SheetData::PoNumber(other.po)) }

        sheet_diffs
    }
}

#[non_exhaustive]
#[derive(Debug, Serialize, Deserialize)]
pub enum SheetData {
    Grade(String),  // TODO: grade struct
    MaterialMaster(String), // TODO: material types enum?
    HeatNumber(String),
    PoNumber(u64),

    // TODO: others (Wbs, Location)
}
