#[derive(Debug, PartialEq, Eq)]
pub enum StagingStatus {
    Empty,
    Partial { staged: usize, unstaged: usize },
    Ready { staged: usize },
}
