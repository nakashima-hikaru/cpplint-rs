#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum IwyuHeader {
    Algorithm,
    Cstdio,
    Functional,
    Iostream,
    Limits,
    List,
    Map,
    Memory,
    Set,
    String,
    Tuple,
    Utility,
    Vector,
}

impl IwyuHeader {
    pub fn as_str(self) -> &'static str {
        match self {
            IwyuHeader::Algorithm => "algorithm",
            IwyuHeader::Cstdio => "cstdio",
            IwyuHeader::Functional => "functional",
            IwyuHeader::Iostream => "iostream",
            IwyuHeader::Limits => "limits",
            IwyuHeader::List => "list",
            IwyuHeader::Map => "map",
            IwyuHeader::Memory => "memory",
            IwyuHeader::Set => "set",
            IwyuHeader::String => "string",
            IwyuHeader::Tuple => "tuple",
            IwyuHeader::Utility => "utility",
            IwyuHeader::Vector => "vector",
        }
    }
}
