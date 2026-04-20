use {
    super::*,
    crate::{Edit, Position},
};

mod history_tests {
    use super::*;

    #[test]
    fn test_new_history_is_empty() {
        let history = History::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_record_and_entries() {
        let mut history = History::new();

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 1), "B")]);

        assert!(!history.is_empty());
        assert_eq!(history.len(), 2);

        let entries = history.entries();
        assert_eq!(entries[0].edits()[0].text(), "A");
        assert_eq!(entries[1].edits()[0].text(), "B");
    }

    #[test]
    fn test_max_entries() {
        let mut history = History::with_max_entries(3);

        for i in 0..5 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }

        assert_eq!(history.len(), 3);

        let entries = history.entries();
        assert_eq!(entries[0].edits()[0].text(), "2");
        assert_eq!(entries[1].edits()[0].text(), "3");
        assert_eq!(entries[2].edits()[0].text(), "4");
    }

    #[test]
    fn test_seq_num_increments() {
        let mut history = History::new();

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);

        let entries = history.entries();
        assert_eq!(entries[0].seq_num(), 1);
        assert_eq!(entries[1].seq_num(), 2);
    }

    #[test]
    fn test_empty_edits_not_recorded() {
        let mut history = History::new();
        history.record(vec![]);
        assert!(history.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut history = History::new();
        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        assert!(!history.is_empty());

        history.clear();
        assert!(history.is_empty());

        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        assert!(history.entries()[0].seq_num() > 1);
    }

    #[test]
    fn test_since() {
        let mut history = History::new();

        for i in 0..5 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }

        let since = history.since(3);
        assert_eq!(since.len(), 2);
        assert_eq!(since[0].seq_num(), 4);
        assert_eq!(since[1].seq_num(), 5);
    }

    #[test]
    fn test_last() {
        let mut history = History::new();
        assert!(history.last().is_none());

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);

        assert_eq!(history.last().unwrap().edits()[0].text(), "B");
    }
}

mod history_extended_tests {
    use super::*;

    #[test]
    fn test_default_is_new() {
        let history = History::default();
        assert!(history.is_empty());
        assert_eq!(history.max_entries(), History::DEFAULT_MAX_ENTRIES);
    }

    #[test]
    fn test_max_entries_accessor() {
        let history = History::new();
        assert_eq!(history.max_entries(), History::DEFAULT_MAX_ENTRIES);

        let history = History::with_max_entries(42);
        assert_eq!(history.max_entries(), 42);
    }

    #[test]
    fn test_with_max_entries_minimum_one() {
        let history = History::with_max_entries(0);
        assert_eq!(history.max_entries(), 1);
    }

    #[test]
    fn test_get_accessor() {
        let mut history = History::new();
        assert!(history.get(0).is_none());

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 1), "B")]);

        assert_eq!(history.get(0).unwrap().edits()[0].text(), "A");
        assert_eq!(history.get(1).unwrap().edits()[0].text(), "B");
        assert!(history.get(2).is_none());
    }

    #[test]
    fn test_set_max_entries() {
        let mut history = History::new();

        for i in 0..10 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }
        assert_eq!(history.len(), 10);

        history.set_max_entries(3);
        assert_eq!(history.max_entries(), 3);
        assert_eq!(history.len(), 3);

        assert_eq!(history.entries()[0].edits()[0].text(), "7");
        assert_eq!(history.entries()[1].edits()[0].text(), "8");
        assert_eq!(history.entries()[2].edits()[0].text(), "9");
    }

    #[test]
    fn test_set_max_entries_minimum_one() {
        let mut history = History::new();
        history.set_max_entries(0);
        assert_eq!(history.max_entries(), 1);
    }

    #[test]
    fn test_set_max_entries_grow() {
        let mut history = History::with_max_entries(3);

        for i in 0..5 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }
        assert_eq!(history.len(), 3);

        history.set_max_entries(100);
        assert_eq!(history.max_entries(), 100);
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_seq_counter() {
        let mut history = History::new();
        assert_eq!(history.seq_counter(), 0);

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        assert_eq!(history.seq_counter(), 1);

        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        assert_eq!(history.seq_counter(), 2);
    }

    #[test]
    fn test_seq_counter_not_reset_on_clear() {
        let mut history = History::new();
        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        assert_eq!(history.seq_counter(), 2);

        history.clear();
        assert_eq!(history.seq_counter(), 2);

        history.record(vec![Edit::insert(Position::new(0, 0), "C")]);
        assert_eq!(history.seq_counter(), 3);
    }

    #[test]
    fn test_seq_counter_not_incremented_for_empty() {
        let mut history = History::new();
        history.record(vec![]);
        assert_eq!(history.seq_counter(), 0);
    }

    #[test]
    fn test_entry_timestamp() {
        let mut history = History::new();
        let before = std::time::Instant::now();

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);

        let after = std::time::Instant::now();
        let entry = history.get(0).unwrap();
        let ts = entry.timestamp();

        assert!(ts >= before);
        assert!(ts <= after);
    }

    #[test]
    fn test_between() {
        let mut history = History::new();

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        let start = std::time::Instant::now();
        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        history.record(vec![Edit::insert(Position::new(0, 0), "C")]);
        let end = std::time::Instant::now();

        let entries = history.between(start, end);
        assert!(entries.len() >= 2);
        for entry in &entries {
            assert!(entry.timestamp() >= start);
            assert!(entry.timestamp() <= end);
        }
    }

    #[test]
    fn test_between_empty_range() {
        let mut history = History::new();
        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);

        let start = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(100))
            .unwrap();
        let end = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(99))
            .unwrap();

        let entries = history.between(start, end);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_since_empty_history() {
        let history = History::new();
        let entries = history.since(0);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_since_all() {
        let mut history = History::new();
        for i in 0..3 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }

        let entries = history.since(0);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_since_none() {
        let mut history = History::new();
        for i in 0..3 {
            history.record(vec![Edit::insert(Position::new(0, 0), i.to_string())]);
        }

        let entries = history.since(100);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_last_on_empty() {
        let history = History::new();
        assert!(history.last().is_none());
    }

    #[test]
    fn test_record_multiple_edits_per_entry() {
        let mut history = History::new();
        history.record(vec![
            Edit::insert(Position::new(0, 0), "A"),
            Edit::insert(Position::new(0, 1), "B"),
            Edit::insert(Position::new(0, 2), "C"),
        ]);

        assert_eq!(history.len(), 1);
        let entry = history.get(0).unwrap();
        assert_eq!(entry.edits().len(), 3);
        assert_eq!(entry.edits()[0].text(), "A");
        assert_eq!(entry.edits()[1].text(), "B");
        assert_eq!(entry.edits()[2].text(), "C");
    }

    #[test]
    fn test_max_entries_one() {
        let mut history = History::with_max_entries(1);

        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);
        assert_eq!(history.len(), 1);

        history.record(vec![Edit::insert(Position::new(0, 0), "B")]);
        assert_eq!(history.len(), 1);
        assert_eq!(history.entries()[0].edits()[0].text(), "B");
    }

    #[test]
    fn test_history_entry_clone() {
        let mut history = History::new();
        history.record(vec![Edit::insert(Position::new(0, 0), "A")]);

        let entry = history.get(0).unwrap();
        let cloned = entry.clone();
        assert_eq!(cloned.edits()[0].text(), entry.edits()[0].text());
        assert_eq!(cloned.seq_num(), entry.seq_num());
        assert_eq!(cloned.timestamp(), entry.timestamp());
    }
}
