use crate::arguments::Argument;
use rand::{seq::SliceRandom, Rng};
use std::str::FromStr;

use trust_dns_client::proto::{
    op::{Edns, Message, Query},
    {rr::Name, rr::RecordType},
};

pub struct Cache {
    template: Vec<u8>,
    id_random: bool,
    qty_pos: usize,
    qty: Vec<RecordType>,
}

#[warn(dead_code)]
impl Cache {
    pub(crate) fn new(argument: &Argument) -> Cache {
        // let domain = argument.domain.clone();
        let mut query_type: Vec<RecordType> = vec![RecordType::A, RecordType::NS, RecordType::SOA];
        let qty = {
            if argument.qty == "" {
                query_type
            } else {
                query_type.clear();
                for x in argument.qty.clone().split(",") {
                    match RecordType::from_str(x) {
                        Ok(v) => query_type.push(v),
                        _ => {}
                    }
                }
                query_type
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
        message.set_recursion_desired(!argument.nord);
        let mut edns = Edns::default();
        // set the max payload to 1232
        // https://dnsflagday.net/2020/
        edns.set_max_payload(1232);
        message.set_edns(edns);
        if let Ok(raw) = message.to_vec() {
            Cache {
                template: raw,
                id_random: argument.id_random,
                qty,
                qty_pos,
            }
        } else {
            panic!("fail to encode to binary")
        }
    }
    fn get_random_id() -> Vec<u8> {
        let mut rng = rand::thread_rng();
        vec![
            rng.gen::<u8>(),
            rng.gen::<u8>(),
        ]
    }
    pub fn build_message(&mut self) -> Vec<u8> {
        let (left, _) = self.template.split_at_mut(2);
        left.copy_from_slice(&Cache::get_random_id().as_slice());
        let qtype: u16 = u16::from(*(self.qty.choose(&mut rand::thread_rng()).unwrap()));
        let temp = qtype.to_be_bytes();
        self.template[self.qty_pos + 1] = temp[0];
        self.template[self.qty_pos + 2] = temp[1];
        self.template.clone()
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
        let data = cache.build_message();
        assert_eq!(data.len(), cache.template.len());
    }
}
