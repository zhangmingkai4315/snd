use crate::arguments::{Argument, Protocol};
use rand::{seq::SliceRandom, Rng};
use std::str::FromStr;

use trust_dns_client::proto::{
    op::{Edns, Message, Query},
    {rr::Name, rr::RecordType},
};

pub struct Cache {
    template: Vec<u8>,
    packet_id_number: u16,
    qty_pos: usize,
    protocol: Protocol,
    qty: Vec<RecordType>,
}

#[warn(dead_code)]
impl Cache {
    pub(crate) fn new(argument: &Argument) -> Cache {
        // let domain = argument.domain.clone();
        let query_type: Vec<RecordType> = vec![RecordType::A];
        let qty = {
            if argument.qty.size() == 0 {
                query_type
            } else {
                argument.qty.0.clone()
            }
        };

        let ref mut message = Message::new();

        let mut query = Query::default();
        let name = match Name::from_str(argument.domain.clone().as_str()) {
            Ok(name) => name,
            Err(e) => panic!("the domain name format is not correct: {}", e.to_string()),
        };
        let qty_pos = 12 + name.len();
        query.set_name(name);
        message.add_query(query);
        message.set_recursion_desired(!argument.disable_rd);
        message.set_checking_disabled(argument.enable_cd);
        if argument.disable_edns == true {
            let mut edns = Edns::default();
            edns.set_dnssec_ok(argument.enable_dnssec);
            edns.set_max_payload(argument.edns_size);
            // set the max payload to 1232
            // https://dnsflagday.net/2020/
            edns.set_max_payload(1232);
            message.set_edns(edns);
        }

        let protocol = argument.protocol.clone();
        if let Ok(mut raw) = message.to_vec() {
            return match protocol {
                Protocol::UDP => Cache {
                    template: raw,
                    packet_id_number: argument.packet_id,
                    qty,
                    qty_pos,
                    protocol,
                },
                Protocol::TCP => {
                    let size = raw.len();
                    let mut raw_with_size: Vec<u8> =
                        [((size & 0xff00) >> 8) as u8, (size & 0x00ff) as u8].to_vec();
                    raw_with_size.append(&mut raw);
                    Cache {
                        template: raw_with_size,
                        packet_id_number: argument.packet_id,
                        qty,
                        qty_pos,
                        protocol,
                    }
                }
            };
        } else {
            panic!("fail to encode to binary")
        }
    }
    fn get_random_id() -> [u8; 2] {
        let mut rng = rand::thread_rng();
        [rng.gen::<u8>(), rng.gen::<u8>()]
    }
    pub fn build_message(&mut self) -> (Vec<u8>, u16) {
        let offset = {
            match self.protocol {
                Protocol::TCP => 2,
                Protocol::UDP => 0,
            }
        };
        let random_id = {
            match self.packet_id_number {
                0 => Cache::get_random_id(),
                _ => self.packet_id_number.to_be_bytes(),
            }
        };
        self.template[offset] = random_id[0];
        self.template[offset + 1] = random_id[1];
        let qtype: u16 = u16::from(*(self.qty.choose(&mut rand::thread_rng()).unwrap()));
        let temp = qtype.to_be_bytes();
        self.template[self.qty_pos + 1 + offset] = temp[0];
        self.template[self.qty_pos + 2 + offset] = temp[1];
        (self.template.clone(), qtype)
    }
}

#[cfg(test)]
mod test {
    use crate::arguments::Argument;
    use crate::cache::Cache;
    use trust_dns_client::proto::op::Message;

    #[test]
    fn test_cache() {
        let arg = Argument::default();
        let mut cache = Cache::new(&arg);
        match Message::from_vec(cache.template.as_slice()) {
            Ok(v) => assert_eq!(v.query_count(), 1),
            _ => {
                assert!(false);
            }
        }
        let (data, qtype) = cache.build_message();
        assert_eq!(data.len(), cache.template.len());
    }
}
