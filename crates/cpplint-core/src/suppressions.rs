use fxhash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    begin: usize,
    end: usize,
}

impl LineRange {
    pub fn new(begin: usize, end: usize) -> Self {
        Self { begin, end }
    }

    pub fn contains(&self, linenum: usize) -> bool {
        self.begin <= linenum && linenum <= self.end
    }

    pub fn contains_range(&self, other: &Self) -> bool {
        self.begin <= other.begin && other.end <= self.end
    }

    pub fn begin(&self) -> usize {
        self.begin
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn set_end(&mut self, end: usize) {
        self.end = end;
    }
}

impl std::fmt::Display for LineRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}-{}]", self.begin, self.end)
    }
}

#[derive(Debug, Default)]
pub struct ErrorSuppressions {
    suppressions: FxHashMap<String, Vec<LineRange>>,
    open_block_suppressions: Vec<Option<(String, usize)>>,
}

impl ErrorSuppressions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.suppressions.clear();
        self.open_block_suppressions.clear();
    }

    pub fn add_suppression(&mut self, category: impl Into<String>, line_range: LineRange) -> bool {
        let category = category.into();
        let suppressed = self.suppressions.entry(category).or_default();
        if suppressed
            .last()
            .is_some_and(|last| last.contains_range(&line_range))
        {
            return false;
        }

        suppressed.push(line_range);
        true
    }

    pub fn add_global_suppression(&mut self, category: impl Into<String>) {
        let _ = self.add_suppression(category, LineRange::new(0, usize::MAX));
    }

    pub fn add_line_suppression(&mut self, category: impl Into<String>, linenum: usize) {
        let _ = self.add_suppression(category, LineRange::new(linenum, linenum));
    }

    pub fn start_block_suppression(&mut self, category: impl Into<String>, linenum: usize) {
        let category = category.into();
        let inserted = self.add_suppression(category.clone(), LineRange::new(linenum, usize::MAX));
        let pending = if inserted {
            self.suppressions
                .get(&category)
                .and_then(|ranges| ranges.len().checked_sub(1))
                .map(|index| (category, index))
        } else {
            None
        };
        self.open_block_suppressions.push(pending);
    }

    pub fn end_block_suppression(&mut self, linenum: usize) {
        while let Some(open_block) = self.open_block_suppressions.pop() {
            if let Some((category, index)) = open_block
                && let Some(range) = self
                    .suppressions
                    .get_mut(&category)
                    .and_then(|ranges| ranges.get_mut(index))
            {
                range.set_end(linenum);
            }
        }
    }

    pub fn get_open_block_start(&mut self) -> Option<usize> {
        while let Some(open_block) = self.open_block_suppressions.pop() {
            if let Some((category, index)) = open_block
                && let Some(begin) = self
                    .suppressions
                    .get(&category)
                    .and_then(|ranges| ranges.get(index))
                    .map(LineRange::begin)
            {
                return Some(begin);
            }
        }
        None
    }

    pub fn peek_open_block_start(&self) -> Option<usize> {
        self.open_block_suppressions
            .iter()
            .rev()
            .find_map(|open_block| {
                let (category, index) = open_block.as_ref()?;
                self.suppressions
                    .get(category)
                    .and_then(|ranges| ranges.get(*index))
                    .map(LineRange::begin)
            })
    }

    pub fn is_suppressed(&self, category: &str, linenum: usize) -> bool {
        self.is_globally_suppressed(linenum)
            || self
                .suppressions
                .get(category)
                .is_some_and(|ranges| ranges.iter().any(|range| range.contains(linenum)))
    }

    pub fn is_globally_suppressed(&self, linenum: usize) -> bool {
        self.suppressions
            .get("")
            .is_some_and(|ranges| ranges.iter().any(|range| range.contains(linenum)))
    }

    pub fn has_open_block(&self) -> bool {
        !self.open_block_suppressions.is_empty()
    }

    pub fn add_default_c_suppressions(&mut self) {
        self.add_global_suppression("readability/casting");
    }

    pub fn add_default_kernel_suppressions(&mut self) {
        self.add_global_suppression("whitespace/tab");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_range_contains_and_formats() {
        let range = LineRange::new(3, 7);
        assert!(range.contains(3));
        assert!(range.contains(5));
        assert!(range.contains(7));
        assert!(!range.contains(2));
        assert!(!range.contains(8));
        assert_eq!(range.to_string(), "[3-7]");
    }

    #[test]
    fn add_line_and_global_suppressions() {
        let mut suppressions = ErrorSuppressions::new();
        suppressions.add_line_suppression("build/include", 12);

        assert!(suppressions.is_suppressed("build/include", 12));
        assert!(!suppressions.is_suppressed("build/include", 11));
    }

    #[test]
    fn empty_category_suppresses_everything() {
        let mut suppressions = ErrorSuppressions::new();
        suppressions.add_global_suppression("");

        assert!(suppressions.is_suppressed("runtime/int", 999));
        assert!(suppressions.is_globally_suppressed(999));
    }

    #[test]
    fn nested_suppression_is_not_duplicated() {
        let mut suppressions = ErrorSuppressions::new();
        assert!(suppressions.add_suppression("build/include", LineRange::new(10, 20)));
        assert!(!suppressions.add_suppression("build/include", LineRange::new(12, 18)));
        assert!(suppressions.is_suppressed("build/include", 15));
        assert!(!suppressions.is_suppressed("build/include", 21));
    }

    #[test]
    fn block_suppression_is_closed_on_end() {
        let mut suppressions = ErrorSuppressions::new();
        suppressions.start_block_suppression("runtime/int", 4);
        assert!(suppressions.has_open_block());
        suppressions.end_block_suppression(9);

        assert!(!suppressions.has_open_block());
        assert!(suppressions.is_suppressed("runtime/int", 4));
        assert!(suppressions.is_suppressed("runtime/int", 9));
        assert!(!suppressions.is_suppressed("runtime/int", 10));
    }

    #[test]
    fn get_open_block_start_discards_duplicate_entries() {
        let mut suppressions = ErrorSuppressions::new();
        suppressions.start_block_suppression("build/include", 5);
        suppressions.start_block_suppression("build/include", 6);

        assert_eq!(suppressions.get_open_block_start(), Some(5));
        assert!(!suppressions.has_open_block());
    }

    #[test]
    fn default_suppressions_match_cpp_behavior() {
        let mut suppressions = ErrorSuppressions::new();
        suppressions.add_default_c_suppressions();
        suppressions.add_default_kernel_suppressions();

        assert!(suppressions.is_suppressed("readability/casting", 1));
        assert!(suppressions.is_suppressed("whitespace/tab", 1));
    }

    #[test]
    fn clear_removes_all_state() {
        let mut suppressions = ErrorSuppressions::new();
        suppressions.add_line_suppression("build/include", 1);
        suppressions.start_block_suppression("runtime/int", 2);
        suppressions.clear();

        assert!(!suppressions.is_suppressed("build/include", 1));
        assert!(!suppressions.has_open_block());
        assert_eq!(suppressions.get_open_block_start(), None);
    }
}
