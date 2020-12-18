use crate::arguments::Argument;
use std::str::FromStr;
use trust_dns_client::proto::op::Message;
use trust_dns_client::proto::rr::DNSClass;
use trust_dns_client::{proto::rr::Name, proto::rr::Record, rr::RecordType};

struct Cache {
    data: Vec<u8>,
    id_random: bool,
    qty: Option<RecordType>,
}

impl Cache {
    fn new(argument: &Argument) -> Cache {
        // let domain = argument.domain.clone();
        let qty = {
            if argument.qty == "" {
                None
            } else {
                match RecordType::from_str(argument.qty.as_str()) {
                    Ok(v) => Some(v),
                    _ => None,
                }
            }
        };

        // let mut message = Message::new();
        // message.set_
        let mut data = Record::new();
        let name = match Name::from_str(argument.domain.clone().as_str()) {
            Ok(name) => name,
            _ => {
                // TODO validation should not stay here
                unreachable!()
            }
        };
        data.set_name(name);
        data.set_dns_class(DNSClass::IN);

        Cache {
            data: argument.domain.clone().into_bytes(),
            id_random: argument.id_random,
            qty,
        }
    }
}
