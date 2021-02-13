use rand::Rng;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::str::FromStr;
use trust_dns_client::proto::{
    op::{Edns, Message, Query},
    {rr::Name, rr::RecordType},
};
use crate::utils::{Argument, Protocol};

pub struct Cache {
    packet_id_number: u16,
    protocol: Protocol,
    cache: Vec<(Vec<u8>, u16)>,
    counter: usize,
    size: usize,
}

#[warn(dead_code)]
impl Cache {
    pub(crate) fn new_from_file(args: &Argument) -> Vec<(Vec<u8>, u16)> {
        let file = args.file.to_owned();
        let mut query_data = vec![];
        if let Ok(lines) = read_lines(file) {
            // Consumes the iterator, returns an (Optional) String
            for line in lines {
                if let Ok(query_type) = line {
                    let mut splitter = query_type.split_whitespace();
                    // let mut domain = "";
                    // let mut qtype = "";
                    let (domain, qtype) = match splitter.next() {
                        Some(d) => match splitter.next() {
                            Some(q) => (d, q),
                            _ => (d, "A"),
                        },
                        _ => {
                            error!("read query file fail");
                            continue;
                        }
                    };
                    let qty = qtype.parse().unwrap();
                    match Cache::build_packet(domain.to_string(), qty, args) {
                        Some(v) => query_data.push((v, u16::from(qty))),
                        _ => continue,
                    }
                }
            }
        }
        query_data
    }
    pub(crate) fn new_from_argument(args: &Argument) -> Vec<(Vec<u8>, u16)> {
        let domain = args.domain.to_owned();
        let qty = args.qty.as_str();
        let mut query_data = vec![];
        let qty = RecordType::from_str(qty).expect("unknown type");
        if let Some(v) = Cache::build_packet(domain, qty, args) {
            query_data.push((v, u16::from(qty)));
        }
        query_data
    }

    pub(crate) fn build_packet(
        domain: String,
        qty: RecordType,
        args: &Argument,
    ) -> Option<Vec<u8>> {
        let ref mut message = Message::new();

        let mut query = Query::default();
        let name = match Name::from_str(domain.clone().as_str()) {
            Ok(name) => name,
            Err(e) => panic!("the domain name format is not correct: {}", e.to_string()),
        };
        query.set_name(name);
        query.set_query_type(qty);
        message.add_query(query);
        message.set_recursion_desired(!args.disable_rd);
        message.set_checking_disabled(args.enable_cd);
        if args.disable_edns == true {
            let mut edns = Edns::default();
            edns.set_dnssec_ok(args.enable_dnssec);
            edns.set_max_payload(args.edns_size);
            message.set_edns(edns);
        }
        let protocol = args.protocol.clone();
        if let Ok(mut raw) = message.to_vec() {
            return match protocol {
                Protocol::UDP | Protocol::DOH => Some(raw),
                Protocol::TCP | Protocol::DOT => {
                    let size = raw.len();
                    let mut raw_with_size: Vec<u8> =
                        [((size & 0xff00) >> 8) as u8, (size & 0x00ff) as u8].to_vec();
                    raw_with_size.append(&mut raw);
                    Some(raw_with_size)
                }
            };
        } else {
            None
        }
    }
    pub(crate) fn new(argument: &Argument) -> Cache {
        // let domain = argument.domain.clone();
        let protocol = argument.clone().protocol;
        let packet_id_number = argument.packet_id;
        if argument.file.is_empty() {
            Cache {
                packet_id_number,
                protocol,
                cache: Cache::new_from_argument(argument),
                counter: 0,
                size: 1,
            }
        } else {
            let cache = Cache::new_from_file(argument);
            let size = cache.len();
            Cache {
                packet_id_number,
                protocol,
                cache,
                counter: 0,
                size,
            }
        }
    }
    fn get_random_id() -> [u8; 2] {
        let mut rng = rand::thread_rng();
        [rng.gen::<u8>(), rng.gen::<u8>()]
    }
    pub fn build_message(&mut self) -> (Vec<u8>, u16) {
        let offset = {
            match self.protocol {
                Protocol::TCP | Protocol::DOT => 2,
                Protocol::UDP | Protocol::DOH => 0,
            }
        };

        let random_id = {
            match self.packet_id_number {
                0 => Cache::get_random_id(),
                _ => self.packet_id_number.to_be_bytes(),
            }
        };

        self.counter += 1;
        let mut data = self.cache[self.counter % self.size].clone();
        data.0[offset] = random_id[0];
        data.0[offset + 1] = random_id[1];
        data
    }
}

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

#[cfg(test)]
mod test {
    use crate::utils::Argument;
    use crate::runner::cache::Cache;
    use trust_dns_client::proto::op::Message;
    #[test]
    fn test_cache() {
        let arg = Argument::default();
        let cache = Cache::new(&arg);
        assert_eq!(cache.cache.len(), 1);
        match Message::from_vec(cache.cache[0].0.as_slice()) {
            Ok(v) => assert_eq!(v.query_count(), 1),
            _ => {
                assert!(false);
            }
        }
    }
}