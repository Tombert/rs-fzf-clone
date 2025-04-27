
#[derive(Clone, Eq, PartialEq)]
pub struct UIStuff {
    pub input: String,
    pub cursor_position: usize,
    pub enter: bool,
}

pub enum Movement {
    Up,
    Down,
    Enter,
}


pub enum Action {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveEnd,
    MoveBegin,
    Exit,
    Select,
    ClearAll,
    BackSpace,
    Other,
    Key(char),
}
