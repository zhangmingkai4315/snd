use core_affinity::get_core_ids;
use core_affinity::CoreId;
use rand::seq::SliceRandom;
use std::error::Error;

pub fn cpu_mode_to_cpu_cores(cpu_mode: String) -> Result<Vec<CoreId>, Box<dyn Error>> {
    let cores = get_core_ids().unwrap();
    let cores_max = cores[cores.len() - 1].id;
    let numbers: Vec<&str> = cpu_mode.split(',').collect();
    return match cpu_mode.to_lowercase().as_str() {
        "all" => Ok(cores),
        "random" => {
            let selected = cores.choose(&mut rand::thread_rng());
            if selected.is_none() {
                Err("random select fail")?
            }
            Ok(vec![*selected.unwrap()])
        }
        _ => {
            let mut result = vec![];
            for i in numbers {
                let core_number = i.parse::<usize>()?;
                if core_number <= cores_max {
                    result.push(cores[core_number])
                } else {
                    Err("error to parse mode")?
                }
            }
            if result.len() == 0 {
                Err("cpu core number is zero")?
            }
            Ok(result)
        }
    };
}

#[test]
fn test_cpu_mode_to_cores() {
    let cores = get_core_ids().unwrap();
    assert_eq!(cpu_mode_to_cpu_cores("random".to_owned()).unwrap().len(), 1);
    assert_eq!(
        cpu_mode_to_cpu_cores("all".to_owned()).unwrap().len(),
        cores.len()
    );
    assert_eq!(
        cpu_mode_to_cpu_cores("0".to_owned())
            .unwrap()
            .into_iter()
            .map(|d| d.id)
            .collect::<Vec<usize>>(),
        vec![cores[0].id]
    );
    if cores.len() > 1 {
        assert_eq!(
            cpu_mode_to_cpu_cores("0,1".to_owned())
                .unwrap()
                .into_iter()
                .map(|d| d.id)
                .collect::<Vec<usize>>(),
            vec![cores[0].id, cores[1].id]
        );
    }
}
