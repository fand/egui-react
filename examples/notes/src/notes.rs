//! The model: notes, the messages that change them, and the reducer.
//!
//! Nothing here knows about egui. That is the point of keeping it in a file of
//! its own: the reducer is a plain function of `(&mut Notes, Msg)`, so it can
//! be read, and tested, without a `Ui` in sight.

use serde::{Deserialize, Serialize};

/// One note. `updated` is a reading of `ui.input(|i| i.time)`, seconds since
/// the app started, so it means the same on native and in a browser.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub updated: f64,
}

impl Note {
    pub fn words(&self) -> usize {
        self.body.split_whitespace().count()
    }
}

/// Every note, and the next id to hand out.
///
/// The counter is part of the saved state so that titles keep counting up
/// across a restart, and so a deleted note's number is never reused.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Notes {
    pub items: Vec<Note>,
    pub next_id: u64,
}

/// Everything that can change the notes.
#[derive(Debug)]
pub enum Msg {
    Add {
        now: f64,
    },
    Delete {
        id: u64,
    },
    /// The body is edited in place through `bind`; this records that it was.
    Touched {
        id: u64,
        now: f64,
    },
}

pub fn reduce(notes: &mut Notes, msg: Msg) {
    match msg {
        Msg::Add { now } => {
            notes.next_id += 1;
            let id = notes.next_id;
            notes.items.push(Note {
                id,
                title: format!("Note {id}"),
                body: String::new(),
                updated: now,
            });
        }
        Msg::Delete { id } => notes.items.retain(|note| note.id != id),
        Msg::Touched { id, now } => {
            if let Some(note) = notes.items.iter_mut().find(|note| note.id == id) {
                note.updated = now;
            }
        }
    }
}

/// The notes a search matches, newest first.
///
/// The id breaks ties, because two notes made in the same frame have the same
/// `updated` and a list that reordered itself between frames would be no use.
pub fn visible(notes: &Notes, search: &str) -> Vec<Note> {
    let search = search.trim().to_lowercase();
    let mut found: Vec<Note> = notes
        .items
        .iter()
        .filter(|note| {
            search.is_empty()
                || note.title.to_lowercase().contains(&search)
                || note.body.to_lowercase().contains(&search)
        })
        .cloned()
        .collect();
    found.sort_by(|a, b| {
        b.updated
            .partial_cmp(&a.updated)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.id.cmp(&a.id))
    });
    found
}
