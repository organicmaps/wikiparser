//! OpenStreetMap types

/// OSM Object Id
///
/// Negative values indicate an updated/created object that has not been sent to the server.
///
/// See <https://wiki.openstreetmap.org/wiki/Elements#Common_attributes>
pub type Id = i64;

/// OSM Object Version
///
/// See <https://wiki.openstreetmap.org/wiki/Elements#Common_attributes>
pub type Version = i32;

/// OSM Object Type
///
/// See <https://wiki.openstreetmap.org/wiki/Elements>
#[derive(Debug, PartialEq, Eq)]
pub enum Kind {
    Node,
    Way,
    Relation,
}

pub fn make_url(obj: Kind, id: Id) -> Option<String> {
    if id < 0 {
        return None;
    }
    Some(format!("https://osm.org/{}/{id}", obj.oname()))
}

impl Kind {
    pub fn from_otype(otype: u8) -> Option<Self> {
        match otype {
            0 => Some(Kind::Node),
            1 => Some(Kind::Way),
            2 => Some(Kind::Relation),
            _ => None,
        }
    }

    pub fn from_oname(oname: &str) -> Option<Self> {
        match oname.trim() {
            "node" => Some(Kind::Node),
            "way" => Some(Kind::Way),
            "relation" => Some(Kind::Relation),
            _ => None,
        }
    }

    pub fn otype(&self) -> u8 {
        match self {
            Kind::Node => 0,
            Kind::Way => 1,
            Kind::Relation => 2,
        }
    }

    pub fn oname(&self) -> &'static str {
        match self {
            Kind::Node => "node",
            Kind::Way => "way",
            Kind::Relation => "relation",
        }
    }
}
