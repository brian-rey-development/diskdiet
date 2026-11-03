use super::layout::fit;
use crate::domain::model::{Category, Item};
use crate::infra::fs::size::format_bytes;
use std::cmp::Reverse;
use std::collections::BTreeMap;

const WEIGHING: &str = "Weighing your disk...";
const SEPARATOR: &str = " · ";

pub struct ScanProgress {
    total_scanners: usize,
    finished: usize,
    only: Vec<Category>,
    bytes_by_category: BTreeMap<Category, u64>,
}

impl ScanProgress {
    pub fn new(total_scanners: usize, only: &[Category]) -> Self {
        Self {
            total_scanners,
            finished: 0,
            only: only.to_vec(),
            bytes_by_category: BTreeMap::new(),
        }
    }

    pub fn record(&mut self, items: &[Item]) {
        self.finished += 1;
        for item in items {
            if self.is_offered(item.category) {
                *self.bytes_by_category.entry(item.category).or_default() += item.bytes;
            }
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.bytes_by_category.values().sum()
    }

    fn is_offered(&self, category: Category) -> bool {
        self.only.is_empty() || self.only.contains(&category)
    }

    fn ranked_categories(&self) -> Vec<(Category, u64)> {
        let mut ranked: Vec<(Category, u64)> = self
            .bytes_by_category
            .iter()
            .map(|(&category, &bytes)| (category, bytes))
            .filter(|&(_, bytes)| bytes > 0)
            .collect();
        ranked.sort_by_key(|&(_, bytes)| Reverse(bytes));
        ranked
    }

    pub fn line(&self, width: usize) -> String {
        let mut parts = vec![format!(
            "{WEIGHING} {}/{}",
            self.finished, self.total_scanners
        )];
        parts.extend(
            self.ranked_categories()
                .into_iter()
                .map(|(category, bytes)| format!("{} {}", category.title(), format_bytes(bytes))),
        );
        fit(&parts.join(SEPARATOR), width)
    }
}
