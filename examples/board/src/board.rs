//! The board: columns, cards, the messages that change them, and the reducer.
//!
//! Nothing here knows about egui or about egui-reactor, and both versions of the
//! example use every line of it. That is deliberate: the two UIs in the gallery
//! then differ only in how they *hold* their state, which is the one thing this
//! example is about.

use serde::{Deserialize, Serialize};

pub type CardId = u64;
pub type ColumnId = u64;

/// One card: a line of text and a tick. The title is the *saved* text; what is
/// being typed into it lives in the card's own UI state and never reaches this
/// file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: CardId,
    pub title: String,
    pub done: bool,
}

/// One column: a name and the cards in it, in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    pub id: ColumnId,
    pub name: String,
    pub cards: Vec<Card>,
}

/// The whole board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub columns: Vec<Column>,
    /// The next id to hand out. Part of the saved state, so a restart never
    /// reuses the id of a card that was deleted.
    next_id: u64,
    /// Bumped by every change [`reduce`] actually makes.
    ///
    /// This is what a `use_memo` hashes instead of the whole board: the deps of
    /// a memo are hashed on every frame, and hashing a few hundred cards sixty
    /// times a second to find out that nothing moved would cost more than the
    /// filtering it is meant to save.
    pub rev: u64,
}

/// Where a dragged card would land: which column, and the card it goes in front
/// of. `before: None` means the end of the column.
///
/// Identity, not an index: a column's indices shift as soon as the dragged card
/// leaves it, and the search filter means the card *above* the pointer is not
/// necessarily the card at that index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DropTarget {
    pub column: ColumnId,
    pub before: Option<CardId>,
}

/// Everything that can change the board.
///
/// One message per user action, including [`Msg::MoveCard`], which covers both
/// halves of a drag: taking a card out of one column and putting it into
/// another position. Splitting it into a remove and an insert would make the
/// undo history take two steps to walk back one drag, and would leave the
/// board in a state with the card in neither column in between.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// Added with its title already typed. A card with an empty title never
    /// reaches the board: the new-card editor lives in the column's own state,
    /// so cancelling it costs no undo step and saves nothing.
    AddCard {
        column: ColumnId,
        title: String,
    },
    SetTitle {
        card: CardId,
        title: String,
    },
    SetDone {
        card: CardId,
        done: bool,
    },
    RemoveCard {
        card: CardId,
    },
    /// `to_index` counts the destination column with the card already taken
    /// out of it. [`Board::drop_index`] works it out from a [`DropTarget`].
    MoveCard {
        card: CardId,
        to_column: ColumnId,
        to_index: usize,
    },
    RenameColumn {
        column: ColumnId,
        name: String,
    },
}

impl Board {
    /// The board the example opens with. The gallery shows an example the
    /// moment it is picked, so an empty one would say nothing.
    pub fn demo() -> Self {
        let mut next_id = 0;
        let mut card = |title: &str, done: bool| {
            next_id += 1;
            Card {
                id: next_id,
                title: title.to_owned(),
                done,
            }
        };
        let columns = vec![
            Column {
                id: 101,
                name: String::from("backlog"),
                cards: vec![
                    card("write the plan", false),
                    card("read the plan", false),
                    card("buy milk", false),
                    card("fix the roof", false),
                ],
            },
            Column {
                id: 102,
                name: String::from("doing"),
                cards: vec![
                    card("draw the board", false),
                    card("wire the drag", false),
                    card("name the hooks", false),
                ],
            },
            Column {
                id: 103,
                name: String::from("done"),
                cards: vec![
                    card("choose a subject", true),
                    card("set the table", true),
                    card("make the tea", true),
                ],
            },
        ];
        Self {
            columns,
            next_id,
            rev: 0,
        }
    }

    /// The card with this id, wherever it is.
    pub fn card(&self, card: CardId) -> Option<&Card> {
        self.columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|c| c.id == card)
    }

    /// Which column a card is in, and where in it.
    pub fn locate(&self, card: CardId) -> Option<(usize, usize)> {
        self.columns.iter().enumerate().find_map(|(ci, column)| {
            let ki = column.cards.iter().position(|c| c.id == card)?;
            Some((ci, ki))
        })
    }

    /// How many cards are on the board. The toolbar shows it.
    pub fn len(&self) -> usize {
        self.columns.iter().map(|column| column.cards.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The `to_index` of the [`Msg::MoveCard`] that lands `card` on `target`.
    ///
    /// Counted with the card already taken out, because that is the order the
    /// reducer does it in: a card moved down inside its own column would
    /// otherwise land one place too low.
    pub fn drop_index(&self, card: CardId, target: DropTarget) -> usize {
        let Some(column) = self.columns.iter().find(|c| c.id == target.column) else {
            return 0;
        };
        let ids: Vec<CardId> = column
            .cards
            .iter()
            .map(|c| c.id)
            .filter(|id| *id != card)
            .collect();
        match target.before {
            Some(before) => ids.iter().position(|id| *id == before).unwrap_or(ids.len()),
            None => ids.len(),
        }
    }
}

/// Apply one message. The only function that changes a board.
pub fn reduce(board: &mut Board, msg: Msg) {
    let changed = match msg {
        Msg::AddCard { column, title } => {
            // The UI drops an empty title before it gets here, but a reducer
            // that can be sent anything should not put a nameless card on the
            // board either: there would be nothing to click on to name it.
            if title.trim().is_empty() {
                return;
            }
            let Some(column) = board.columns.iter_mut().find(|c| c.id == column) else {
                return;
            };
            board.next_id += 1;
            column.cards.push(Card {
                id: board.next_id,
                title,
                done: false,
            });
            true
        }
        Msg::SetTitle { card, title } => {
            let Some((ci, ki)) = board.locate(card) else {
                return;
            };
            let card = &mut board.columns[ci].cards[ki];
            let changed = card.title != title;
            card.title = title;
            changed
        }
        Msg::SetDone { card, done } => {
            let Some((ci, ki)) = board.locate(card) else {
                return;
            };
            let card = &mut board.columns[ci].cards[ki];
            let changed = card.done != done;
            card.done = done;
            changed
        }
        Msg::RemoveCard { card } => {
            let Some((ci, ki)) = board.locate(card) else {
                return;
            };
            board.columns[ci].cards.remove(ki);
            true
        }
        Msg::MoveCard {
            card,
            to_column,
            to_index,
        } => {
            let Some((ci, ki)) = board.locate(card) else {
                return;
            };
            let Some(di) = board.columns.iter().position(|c| c.id == to_column) else {
                return;
            };
            if ci == di && ki == to_index {
                return;
            }
            let card = board.columns[ci].cards.remove(ki);
            let to_index = to_index.min(board.columns[di].cards.len());
            board.columns[di].cards.insert(to_index, card);
            true
        }
        Msg::RenameColumn { column, name } => {
            let Some(column) = board.columns.iter_mut().find(|c| c.id == column) else {
                return;
            };
            let changed = column.name != name;
            column.name = name;
            changed
        }
    };
    if changed {
        board.rev += 1;
    }
}

/// The cards of one column that the search text and the done filter leave
/// visible, in order.
///
/// `done`: `None` shows everything, `Some(true)` only the ticked cards,
/// `Some(false)` only the ones still open.
pub fn visible(column: &Column, search: &str, done: Option<bool>) -> Vec<CardId> {
    let search = search.trim().to_lowercase();
    column
        .cards
        .iter()
        .filter(|card| done.is_none_or(|done| card.done == done))
        .filter(|card| search.is_empty() || card.title.to_lowercase().contains(&search))
        .map(|card| card.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A board of two columns whose cards are `1, 2, 3` and `4, 5`. Card 2 is
    /// the only one that is done.
    fn board() -> Board {
        let card = |id: CardId, title: &str| Card {
            id,
            title: title.to_owned(),
            done: id == 2,
        };
        Board {
            columns: vec![
                Column {
                    id: 10,
                    name: String::from("left"),
                    cards: vec![card(1, "alpha"), card(2, "beta"), card(3, "gamma")],
                },
                Column {
                    id: 20,
                    name: String::from("right"),
                    cards: vec![card(4, "delta"), card(5, "epsilon")],
                },
            ],
            next_id: 5,
            rev: 0,
        }
    }

    fn ids(board: &Board, column: ColumnId) -> Vec<CardId> {
        board
            .columns
            .iter()
            .find(|c| c.id == column)
            .expect("the column")
            .cards
            .iter()
            .map(|card| card.id)
            .collect()
    }

    /// Move `card` to where `target` points, the way the UI does it.
    fn drop_on(board: &mut Board, card: CardId, target: DropTarget) {
        let to_index = board.drop_index(card, target);
        reduce(
            board,
            Msg::MoveCard {
                card,
                to_column: target.column,
                to_index,
            },
        );
    }

    fn add(board: &mut Board, column: ColumnId, title: &str) {
        reduce(
            board,
            Msg::AddCard {
                column,
                title: title.to_owned(),
            },
        );
    }

    #[test]
    fn a_card_moves_forwards_inside_its_column() {
        let mut board = board();
        // 1 goes in front of 3: the index is counted with 1 already out, so
        // `[2, 3]` puts it at 1 and not at 2.
        drop_on(
            &mut board,
            1,
            DropTarget {
                column: 10,
                before: Some(3),
            },
        );
        assert_eq!(ids(&board, 10), vec![2, 1, 3]);
    }

    #[test]
    fn a_card_moves_backwards_inside_its_column() {
        let mut board = board();
        drop_on(
            &mut board,
            3,
            DropTarget {
                column: 10,
                before: Some(1),
            },
        );
        assert_eq!(ids(&board, 10), vec![3, 1, 2]);
    }

    #[test]
    fn a_card_moves_to_the_end_of_its_column() {
        let mut board = board();
        drop_on(
            &mut board,
            1,
            DropTarget {
                column: 10,
                before: None,
            },
        );
        assert_eq!(ids(&board, 10), vec![2, 3, 1]);
    }

    #[test]
    fn a_card_moves_to_another_column() {
        let mut board = board();
        drop_on(
            &mut board,
            2,
            DropTarget {
                column: 20,
                before: Some(5),
            },
        );
        assert_eq!(ids(&board, 10), vec![1, 3]);
        assert_eq!(ids(&board, 20), vec![4, 2, 5]);
    }

    #[test]
    fn a_move_that_changes_nothing_does_not_bump_the_revision() {
        let mut board = board();
        let rev = board.rev;
        drop_on(
            &mut board,
            1,
            DropTarget {
                column: 10,
                before: Some(2),
            },
        );
        assert_eq!(ids(&board, 10), vec![1, 2, 3]);
        assert_eq!(board.rev, rev, "nothing moved, so nothing changed");
    }

    #[test]
    fn adding_and_removing_cards() {
        let mut board = board();
        add(&mut board, 20, "zeta");
        assert_eq!(ids(&board, 20), vec![4, 5, 6]);
        assert_eq!(board.card(6).expect("the new card").title, "zeta");
        reduce(&mut board, Msg::RemoveCard { card: 4 });
        assert_eq!(ids(&board, 20), vec![5, 6]);
        // The id of the removed card is not handed out again.
        add(&mut board, 20, "eta");
        assert_eq!(ids(&board, 20), vec![5, 6, 7]);
    }

    #[test]
    fn a_card_with_an_empty_title_is_not_added() {
        let mut board = board();
        let rev = board.rev;
        add(&mut board, 20, "");
        add(&mut board, 20, "   ");
        assert_eq!(ids(&board, 20), vec![4, 5]);
        assert_eq!(board.rev, rev, "and the id is not spent either");
    }

    #[test]
    fn setting_the_title_to_what_it_already_says_does_not_bump_the_revision() {
        let mut board = board();
        reduce(
            &mut board,
            Msg::SetTitle {
                card: 1,
                title: String::from("alpha"),
            },
        );
        assert_eq!(board.rev, 0, "nothing to walk back");
        reduce(
            &mut board,
            Msg::SetTitle {
                card: 1,
                title: String::from("alpha!"),
            },
        );
        assert_eq!(board.card(1).expect("the card").title, "alpha!");
        assert_eq!(board.rev, 1);
    }

    #[test]
    fn ticking_a_card_is_one_change() {
        let mut board = board();
        reduce(
            &mut board,
            Msg::SetDone {
                card: 1,
                done: true,
            },
        );
        assert!(board.card(1).expect("the card").done);
        assert_eq!(board.rev, 1);
        // Ticking it again says the same thing, so the history stays put.
        reduce(
            &mut board,
            Msg::SetDone {
                card: 1,
                done: true,
            },
        );
        assert_eq!(board.rev, 1);
    }

    #[test]
    fn the_search_looks_at_the_title() {
        let board = board();
        let column = &board.columns[0];
        assert_eq!(visible(column, "", None), vec![1, 2, 3]);
        assert_eq!(visible(column, "  BET ", None), vec![2]);
        assert_eq!(visible(column, "a", None), vec![1, 2, 3]);
        assert_eq!(visible(column, "nothing", None), Vec::<CardId>::new());
    }

    #[test]
    fn the_done_filter_and_the_search_narrow_together() {
        let board = board();
        let column = &board.columns[0];
        assert_eq!(visible(column, "", Some(true)), vec![2]);
        assert_eq!(visible(column, "", Some(false)), vec![1, 3]);
        assert_eq!(visible(column, "alpha", Some(true)), Vec::<CardId>::new());
        assert_eq!(
            visible(&board.columns[1], "", Some(true)),
            Vec::<CardId>::new()
        );
    }

    #[test]
    fn the_demo_board_has_a_done_column() {
        let board = Board::demo();
        assert_eq!(board.len(), 10);
        let done = board.columns.last().expect("the last column");
        assert_eq!(done.name, "done");
        assert!(done.cards.iter().all(|card| card.done));
        assert!(
            board.columns[..2]
                .iter()
                .flat_map(|column| column.cards.iter())
                .all(|card| !card.done)
        );
    }
}
