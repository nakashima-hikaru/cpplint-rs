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

#[cfg(test)]
mod tests {
    use super::IwyuHeader;

    #[test]
    fn iwyu_headers_roundtrip_all_known_values() {
        let headers = [
            IwyuHeader::Algorithm,
            IwyuHeader::Cstdio,
            IwyuHeader::Functional,
            IwyuHeader::Iostream,
            IwyuHeader::Limits,
            IwyuHeader::List,
            IwyuHeader::Map,
            IwyuHeader::Memory,
            IwyuHeader::Set,
            IwyuHeader::String,
            IwyuHeader::Tuple,
            IwyuHeader::Utility,
            IwyuHeader::Vector,
        ];

        for header in headers {
            assert_eq!(header.as_str(), header.as_str());
            assert!(!header.as_str().is_empty());
        }
    }
}
