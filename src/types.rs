
#[derive(Clone, Eq, PartialEq)]
pub struct UIStuff {
    input: String,
    cursor_position: usize,
    enter: bool,
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
