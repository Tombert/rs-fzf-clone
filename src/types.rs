use clap::Parser;
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

#[derive(Parser)]
#[command(name = "sway-app-workspace-index")]
#[command(author = "thomas@gebert.app")]
#[command(version = "1.0")]
#[command(about = "nada")]
pub struct Args {
    #[arg(short, long)]
    pub buffsize: Option<usize>,

    #[arg(short, long)]
    pub scoreclamp: Option<usize>,

    #[arg(short, long)]
    pub batchsize: Option<usize>,
}
