//! The tape of blocks: a block's place, and the edits of the Script Builder (insert, remove, move,
//! undo).

use super::Block;

/// (body, index) steps from the top list: `[(0, i)]` is a top block; `(b, j)` inside a compound
/// block is its `b`-th body (`bodies` order), `j`-th block.
pub type Pos = Vec<(usize, usize)>;

/// A block's lists of blocks: If: branches…, else; Try: body, handlers…, finally (when there is
/// one); For / While / With / Def: body.
pub fn bodies(b: &Block) -> Vec<&Vec<Block>> {
    match b {
        Block::If { branches, orelse } => branches.iter().map(|(_, b)| b).chain([orelse]).collect(),
        Block::Try {
            body,
            handlers,
            finally,
        } => std::iter::once(body)
            .chain(handlers.iter().map(|h| &h.body))
            .chain(finally.as_ref())
            .collect(),
        Block::For { body, .. }
        | Block::While { body, .. }
        | Block::With { body, .. }
        | Block::Def { body, .. }
        | Block::ForHost { body, .. } => {
            vec![body]
        }
        _ => vec![],
    }
}

pub fn bodies_mut(b: &mut Block) -> Vec<&mut Vec<Block>> {
    match b {
        Block::If { branches, orelse } => branches
            .iter_mut()
            .map(|(_, b)| b)
            .chain([orelse])
            .collect(),
        Block::Try {
            body,
            handlers,
            finally,
        } => std::iter::once(body)
            .chain(handlers.iter_mut().map(|h| &mut h.body))
            .chain(finally.as_mut())
            .collect(),
        Block::For { body, .. }
        | Block::While { body, .. }
        | Block::With { body, .. }
        | Block::Def { body, .. }
        | Block::ForHost { body, .. } => {
            vec![body]
        }
        _ => vec![],
    }
}

/// The list holding the block at `at` (`at` without its last step).
fn list_mut<'a>(top: &'a mut Vec<Block>, at: &[(usize, usize)]) -> Option<&'a mut Vec<Block>> {
    let (_, parent) = at.split_last()?;
    let mut list = top;
    for (i, &(_, idx)) in parent.iter().enumerate() {
        let body = at[i + 1].0;
        list = bodies_mut(list.get_mut(idx)?).into_iter().nth(body)?;
    }
    Some(list)
}

pub fn get<'a>(top: &'a [Block], at: &[(usize, usize)]) -> Option<&'a Block> {
    let (&(_, first), rest) = at.split_first()?;
    let mut b = top.get(first)?;
    for &(body, idx) in rest {
        b = bodies(b).into_iter().nth(body)?.get(idx)?;
    }
    Some(b)
}

pub fn get_mut<'a>(top: &'a mut Vec<Block>, at: &[(usize, usize)]) -> Option<&'a mut Block> {
    let &(_, idx) = at.last()?;
    list_mut(top, at)?.get_mut(idx)
}

/// Puts `b` at `at` (its index may be the list's length: at the end). `false`: no such place.
pub fn insert(top: &mut Vec<Block>, at: &[(usize, usize)], b: Block) -> bool {
    let Some(&(_, idx)) = at.last() else {
        return false;
    };
    match list_mut(top, at) {
        Some(list) if idx <= list.len() => {
            list.insert(idx, b);
            true
        }
        _ => false,
    }
}

pub fn remove(top: &mut Vec<Block>, at: &[(usize, usize)]) -> Option<Block> {
    let &(_, idx) = at.last()?;
    let list = list_mut(top, at)?;
    (idx < list.len()).then(|| list.remove(idx))
}

/// Moves the block at `from` to `to` (a place counted before the move); its new place, `None`
/// when refused (into itself, no such place) — the tape is then as it was.
pub fn move_to(
    top: &mut Vec<Block>,
    from: &[(usize, usize)],
    to: &[(usize, usize)],
) -> Option<Pos> {
    if to.len() > from.len() && to.starts_with(from) {
        return None;
    }
    let mut to = to.to_vec();
    // Taking `from` out shifts what follows it in the same list, here or above `to`'s place.
    let d = from.len() - 1;
    if to.len() > d && to[..d] == from[..d] && to[d].0 == from[d].0 && to[d].1 > from[d].1 {
        to[d].1 -= 1;
    }
    let saved = top.clone();
    let b = remove(top, from)?;
    if insert(top, &to, b) {
        Some(to)
    } else {
        *top = saved;
        None
    }
}

/// The tape being edited, with its undo and redo.
#[derive(Debug, Default, Clone)]
pub struct Doc {
    pub blocks: Vec<Block>,
    // ponytail: a copy of the tape per edit, diffs if scripts get huge
    undo: Vec<Vec<Block>>,
    redo: Vec<Vec<Block>>,
}

impl Doc {
    pub fn new(blocks: Vec<Block>) -> Self {
        Self {
            blocks,
            undo: vec![],
            redo: vec![],
        }
    }

    /// An edit: `f` says whether it changed anything (only then can it be undone).
    pub fn edit(&mut self, f: impl FnOnce(&mut Vec<Block>) -> bool) -> bool {
        let before = self.blocks.clone();
        if !f(&mut self.blocks) {
            self.blocks = before;
            return false;
        }
        self.undo.push(before);
        self.redo.clear();
        true
    }

    pub fn undo(&mut self) -> bool {
        let Some(b) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.blocks, b));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(b) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.blocks, b));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(t: &str) -> Block {
        Block::Comment { text: t.into() }
    }

    fn texts(l: &[Block]) -> Vec<String> {
        l.iter()
            .map(|b| match b {
                Block::Comment { text } => text.clone(),
                _ => "#".into(),
            })
            .collect()
    }

    /// a, for[ b, c ], if[ d ] else [ e ]
    fn tape() -> Vec<Block> {
        vec![
            c("a"),
            Block::For {
                target: "x".into(),
                iter: "y".into(),
                body: vec![c("b"), c("c")],
            },
            Block::If {
                branches: vec![("z".into(), vec![c("d")])],
                orelse: vec![c("e")],
            },
        ]
    }

    #[test]
    fn get_nested() {
        let t = tape();
        assert_eq!(get(&t, &[(0, 1), (0, 1)]), Some(&c("c")));
        assert_eq!(get(&t, &[(0, 2), (1, 0)]), Some(&c("e")));
        assert_eq!(get(&t, &[(0, 2), (2, 0)]), None);
        assert_eq!(get(&t, &[]), None);
    }

    #[test]
    fn insert_end_and_middle() {
        let mut t = tape();
        assert!(insert(&mut t, &[(0, 1), (0, 2)], c("end")));
        assert!(insert(&mut t, &[(0, 0)], c("first")));
        assert!(!insert(&mut t, &[(0, 9)], c("no")));
        assert_eq!(texts(bodies(&t[2])[0]), ["b", "c", "end"]);
        assert_eq!(texts(&t[..1]), ["first"]);
    }

    #[test]
    fn remove_shifts() {
        let mut t = tape();
        assert_eq!(remove(&mut t, &[(0, 1), (0, 0)]), Some(c("b")));
        assert_eq!(texts(bodies(&t[1])[0]), ["c"]);
        assert_eq!(remove(&mut t, &[(0, 1), (0, 5)]), None);
    }

    #[test]
    fn move_down_same_list() {
        let mut t = tape();
        // a to just before the if (index 2 counted before the move)
        assert_eq!(move_to(&mut t, &[(0, 0)], &[(0, 2)]), Some(vec![(0, 1)]));
        assert!(matches!(t[0], Block::For { .. }));
        assert_eq!(t[1], c("a"));
    }

    #[test]
    fn move_into_other_body() {
        let mut t = tape();
        assert_eq!(
            move_to(&mut t, &[(0, 0)], &[(0, 2), (1, 1)]),
            Some(vec![(0, 1), (1, 1)])
        );
        assert_eq!(texts(bodies(&t[1])[1]), ["e", "a"]);
        // out of a body, to the top
        assert_eq!(
            move_to(&mut t, &[(0, 0), (0, 0)], &[(0, 0)]),
            Some(vec![(0, 0)])
        );
        assert_eq!(t[0], c("b"));
    }

    #[test]
    fn move_into_self_refused() {
        let mut t = tape();
        let before = t.clone();
        assert_eq!(move_to(&mut t, &[(0, 1)], &[(0, 1), (0, 0)]), None);
        assert_eq!(move_to(&mut t, &[(0, 0)], &[(0, 7)]), None);
        assert_eq!(t, before);
    }

    #[test]
    fn undo_redo() {
        let mut d = Doc::new(tape());
        assert!(!d.edit(|t| {
            t.clear();
            false
        }));
        assert_eq!(d.blocks.len(), 3, "a refused edit changes nothing");
        assert!(!d.undo());
        assert!(d.edit(|t| remove(t, &[(0, 0)]).is_some()));
        assert_eq!(d.blocks.len(), 2);
        assert!(d.undo());
        assert_eq!(d.blocks.len(), 3);
        assert!(d.redo());
        assert_eq!(d.blocks.len(), 2);
        assert!(d.undo());
        assert!(d.edit(|t| insert(t, &[(0, 0)], c("n"))));
        assert!(!d.redo(), "a new edit clears redo");
    }
}
