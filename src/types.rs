use crate::bindings;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum WrenType {
    Bool,
    Number,
    Foreign,
    List,
    Null,
    String,
    Unknown,
}

impl Into<bindings::WrenType> for WrenType {
    #[rustfmt::skip]
    fn into(self) -> bindings::WrenType {
        match self {
            WrenType::Bool    => bindings::WrenType_WREN_TYPE_BOOL,
            WrenType::Number  => bindings::WrenType_WREN_TYPE_NUM,
            WrenType::Foreign => bindings::WrenType_WREN_TYPE_FOREIGN,
            WrenType::List    => bindings::WrenType_WREN_TYPE_LIST,
            WrenType::Null    => bindings::WrenType_WREN_TYPE_NULL,
            WrenType::String  => bindings::WrenType_WREN_TYPE_STRING,
            WrenType::Unknown => bindings::WrenType_WREN_TYPE_UNKNOWN,
        }
    }
}

impl From<bindings::WrenType> for WrenType {
    #[rustfmt::skip]
    fn from(other: bindings::WrenType) -> Self {
        match other {
            bindings::WrenType_WREN_TYPE_BOOL    => WrenType::Bool,
            bindings::WrenType_WREN_TYPE_NUM     => WrenType::Number,
            bindings::WrenType_WREN_TYPE_FOREIGN => WrenType::Foreign,
            bindings::WrenType_WREN_TYPE_LIST    => WrenType::List,
            bindings::WrenType_WREN_TYPE_NULL    => WrenType::Null,
            bindings::WrenType_WREN_TYPE_STRING  => WrenType::String,
            bindings::WrenType_WREN_TYPE_UNKNOWN => WrenType::Unknown,
            _ => unreachable!(),
        }
    }
}
