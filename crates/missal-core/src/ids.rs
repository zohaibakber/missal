use std::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(i64);

        impl $name {
            pub const fn new(value: i64) -> Option<Self> {
                if value >= 1 { Some(Self(value)) } else { None }
            }

            pub const fn get(self) -> i64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(FirId);
id_type!(TemplateId);
id_type!(FirDocumentId);
id_type!(PlaceholderId);
id_type!(Revision);

impl Revision {
    pub const FIRST: Revision = Revision(1);

    pub const fn next(self) -> Revision {
        Revision(self.0 + 1)
    }
}
