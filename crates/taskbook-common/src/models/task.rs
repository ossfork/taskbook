use serde::{Deserialize, Serialize};

use super::item::Item;
use crate::board;

/// A task item with completion status and priority
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(rename = "_id")]
    pub id: u64,

    #[serde(rename = "_date")]
    pub date: String,

    #[serde(rename = "_timestamp")]
    pub timestamp: i64,

    #[serde(rename = "_isTask")]
    pub is_task_flag: bool,

    pub description: String,

    #[serde(rename = "isStarred")]
    pub is_starred: bool,

    #[serde(rename = "isComplete")]
    pub is_complete: bool,

    #[serde(rename = "inProgress")]
    pub in_progress: bool,

    pub priority: u8,

    #[serde(rename = "_dueDate", default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<i64>,

    #[serde(deserialize_with = "board::deserialize_boards")]
    pub boards: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Task {
    /// Creates a new task. The `priority` value is clamped silently to the range 1-3.
    pub fn new(id: u64, description: String, boards: Vec<String>, priority: u8) -> Self {
        let now = chrono::Local::now();
        Self {
            id,
            date: now.format("%a %b %d %Y").to_string(),
            timestamp: now.timestamp_millis(),
            is_task_flag: true,
            description,
            is_starred: false,
            is_complete: false,
            in_progress: false,
            priority: priority.clamp(1, 3),
            due_date: None,
            boards,
            tags: Vec::new(),
        }
    }

    /// Returns the task with the given due date set (epoch millis).
    pub fn with_due_date(mut self, due_date: Option<i64>) -> Self {
        self.due_date = due_date;
        self
    }

    /// Creates a new task with tags.
    pub fn new_with_tags(
        id: u64,
        description: String,
        boards: Vec<String>,
        priority: u8,
        tags: Vec<String>,
    ) -> Self {
        let mut task = Self::new(id, description, boards, priority);
        task.tags = tags;
        task
    }
}

impl Item for Task {
    fn id(&self) -> u64 {
        self.id
    }

    fn date(&self) -> &str {
        &self.date
    }

    fn timestamp(&self) -> i64 {
        self.timestamp
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn is_starred(&self) -> bool {
        self.is_starred
    }

    fn boards(&self) -> &[String] {
        &self.boards
    }

    fn tags(&self) -> &[String] {
        &self.tags
    }

    fn is_task(&self) -> bool {
        self.is_task_flag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_is_task_uses_flag() {
        let task = Task::new(1, "Test".to_string(), vec!["My Board".to_string()], 1);
        assert!(task.is_task());
        assert!(task.is_task_flag);
    }

    #[test]
    fn test_priority_clamped_to_range() {
        let low = Task::new(1, "Test".to_string(), vec!["My Board".to_string()], 0);
        assert_eq!(low.priority, 1);

        let high = Task::new(2, "Test".to_string(), vec!["My Board".to_string()], 255);
        assert_eq!(high.priority, 3);

        let mid = Task::new(3, "Test".to_string(), vec!["My Board".to_string()], 2);
        assert_eq!(mid.priority, 2);
    }

    #[test]
    fn test_due_date_serde_round_trip() {
        let task = Task::new(1, "T".to_string(), vec!["My Board".to_string()], 1)
            .with_due_date(Some(1_790_000_000_000));
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("_dueDate"));
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(back.due_date, Some(1_790_000_000_000));
    }

    #[test]
    fn test_task_without_due_date_omits_field_and_deserializes() {
        let task = Task::new(1, "T".to_string(), vec!["My Board".to_string()], 1);
        let json = serde_json::to_string(&task).unwrap();
        assert!(!json.contains("_dueDate"));
        // Legacy JSON without the field still parses
        let legacy = r#"{"_id":1,"_date":"Fri Jun 12 2026","_timestamp":1,"_isTask":true,"description":"x","isStarred":false,"isComplete":false,"inProgress":false,"priority":1,"boards":["My Board"]}"#;
        let back: Task = serde_json::from_str(legacy).unwrap();
        assert_eq!(back.due_date, None);
    }
}
