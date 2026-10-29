//! Document annotations use normalized page coordinates, independent of zoom and raster size.
#[derive(Clone, Debug, PartialEq)]
pub struct TextWord {
    pub text: String,
    pub bounds: [f32; 4],
}
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub id: i64,
    pub page: u32,
    pub text: String,
    pub note: String,
    /// None denotes a page bookmark; highlights have one of four colors.
    pub color: Option<u8>,
    pub bounds: Vec<[f32; 4]>,
}
impl Annotation {
    pub fn validate(&self) -> Result<(), String> {
        if self.page == 0
            || self.color.is_some_and(|c| c > 3)
            || (self.color.is_some() && (self.bounds.is_empty() || self.text.is_empty()))
            || self.bounds.iter().any(|b| {
                !b.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                    || b[0] >= b[2]
                    || b[1] >= b[3]
            })
        {
            return Err("Invalid annotation.".into());
        }
        Ok(())
    }
}
pub fn export_markdown(items: &[Annotation]) -> String {
    let mut out = String::from("# Reading annotations\n\n");
    for item in items {
        out.push_str(&format!(
            "## Page {} — {}\n\n",
            item.page,
            if item.color.is_none() {
                "Bookmark"
            } else {
                "Highlight"
            }
        ));
        for line in item.text.lines() {
            out.push_str(&format!("> {line}\n"));
        }
        if !item.note.is_empty() {
            out.push_str(&format!("\nNote: {}\n", item.note));
        }
        out.push('\n');
    }
    out
}
