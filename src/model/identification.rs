//! Identification.xsd, PersonName.xsd, ContactInformation.xsd, Demographics.xsd.
use super::label::{date_node, Label};
use crate::date::DateAttrs;
use crate::xml::node::{opt_text, Node};

#[derive(Debug, Clone, Default)]
pub struct PersonName {
    pub first_name: Option<String>,
    pub surname: Option<String>,
}

impl PersonName {
    // PersonName.xsd PersonNameType: Title?, FirstName?, Surname? (Title never emitted)
    pub fn to_node(&self) -> Option<Node> {
        Node::parent(
            "PersonName",
            vec![
                opt_text("FirstName", &self.first_name),
                opt_text("Surname", &self.surname),
            ],
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Address {
    pub line1: Option<String>,
    pub line2: Option<String>,
    pub postal_code: Option<String>,
    pub municipality: Option<String>,
    pub country: Option<Label>,
}

impl Address {
    // ContactInformation.xsd Address > Contact: AddressLine?, AddressLine2?, PostalCode?, Municipality?, Country?
    pub fn to_node(&self) -> Option<Node> {
        let contact = Node::parent(
            "Contact",
            vec![
                opt_text("AddressLine", &self.line1),
                opt_text("AddressLine2", &self.line2),
                opt_text("PostalCode", &self.postal_code),
                opt_text("Municipality", &self.municipality),
                self.country.as_ref().and_then(|c| c.to_node("Country")),
            ],
        );
        Node::parent("Address", vec![contact])
    }
}

#[derive(Debug, Clone)]
pub struct Telephone {
    pub number: String,
    pub use_code: Option<Label>,
}

#[derive(Debug, Clone)]
pub struct Website {
    pub url: String,
    pub use_: Option<Label>,
}

#[derive(Debug, Clone, Default)]
pub struct ContactInfo {
    pub address: Option<Address>,
    pub email: Option<String>,
    pub phones: Vec<Telephone>,
    pub websites: Vec<Website>,
}

impl ContactInfo {
    // ContactInformation.xsd ContactInfoType: Address?, Email?, TelephoneList?, WebsiteList? (IM and ContactMethod never emitted)
    pub fn to_node(&self) -> Option<Node> {
        let email = self
            .email
            .as_ref()
            .and_then(|e| Node::parent("Email", vec![Some(Node::text("Contact", e.clone()))]));
        let phones = Node::parent(
            "TelephoneList",
            self.phones
                .iter()
                .map(|p| {
                    Node::parent(
                        "Telephone",
                        vec![
                            Some(Node::text("Contact", p.number.clone())),
                            p.use_code.as_ref().and_then(|u| u.to_node("Use")),
                        ],
                    )
                })
                .collect(),
        );
        let sites = Node::parent(
            "WebsiteList",
            self.websites
                .iter()
                .map(|w| {
                    Node::parent(
                        "Website",
                        vec![
                            Some(Node::text("Contact", w.url.clone())),
                            w.use_.as_ref().and_then(|u| u.to_node("Use")),
                        ],
                    )
                })
                .collect(),
        );
        Node::parent(
            "ContactInfo",
            vec![
                self.address.as_ref().and_then(|a| a.to_node()),
                email,
                phones,
                sites,
            ],
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Demographics {
    pub birthdate: Option<DateAttrs>,
    pub gender: Option<Label>,
    pub nationalities: Vec<Label>,
}

impl Demographics {
    // Demographics.xsd: Birthdate?, Gender?, NationalityList?
    pub fn to_node(&self) -> Option<Node> {
        Node::parent(
            "Demographics",
            vec![
                self.birthdate.as_ref().map(|d| date_node("Birthdate", d)),
                self.gender.as_ref().and_then(|g| g.to_node("Gender")),
                Node::parent(
                    "NationalityList",
                    self.nationalities
                        .iter()
                        .map(|n| n.to_node("Nationality"))
                        .collect(),
                ),
            ],
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Identification {
    pub name: Option<PersonName>,
    pub contact: Option<ContactInfo>,
    pub demographics: Option<Demographics>,
}

impl Identification {
    // Identification.xsd: PersonName?, ContactInfo?, Demographics? (Photo and Signature never emitted)
    pub fn to_node(&self) -> Option<Node> {
        Node::parent(
            "Identification",
            vec![
                self.name.as_ref().and_then(|n| n.to_node()),
                self.contact.as_ref().and_then(|c| c.to_node()),
                self.demographics.as_ref().and_then(|d| d.to_node()),
            ],
        )
    }
}
