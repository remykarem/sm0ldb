use core::panic;
use std::{
    fs::File,
    io::{self, Read, Write},
    process::Command,
};

const PAGE_SIZE: usize = 256;

const POS_IS_THERE_SPACE: usize = PAGE_SIZE - 1;
const POS_NEXT_ORDER: usize = PAGE_SIZE - 2;
const POS_NUM_RECORDS: usize = PAGE_SIZE - 3;

const POS_START_OF_SLOTS: usize = PAGE_SIZE - 17;
const POS_START_RECORDS: usize = 16;
const RECORD_SIZE: usize = 8;
const SLOT_SIZE: usize = 2;
const POS_NULL: u8 = 0;

const MAX_NUM_RECORDS: usize = 22; // (256-16-16) > 22*(8+2)
const BOUNDARY: usize = POS_START_RECORDS + RECORD_SIZE * MAX_NUM_RECORDS;

fn main() {
    interpreter();
}

fn interpreter() {
    let mut global_rid = 0;
    loop {
        // read input
        println!();
        let mut instruction = String::new();
        print!("sm0ldb> ");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut instruction).unwrap();

        // read file
        let mut page = [0u8; PAGE_SIZE];
        let mut file = File::open("hello").unwrap();
        file.read(&mut page).unwrap();

        if let Some(_) = instruction.strip_prefix("fullscan") {
            do_fullscan(&page);
        }

        if let Some(_) = instruction.strip_prefix("restart") {
            global_rid = 0;
            let empty_page = do_restart();
            let mut file = File::create("hello").unwrap();
            file.write(&empty_page).unwrap();
        }

        if let Some(input) = instruction.trim().strip_prefix("insert ") {
            if input.len() > RECORD_SIZE {
                println!("Cannot...");
            } else {
                println!("{}", global_rid);
                insert(&mut page, &input, global_rid);
                global_rid += 1;
            }
        }

        if let Some(request_rid_str) = instruction.strip_prefix("read ") {
            let request_rid = request_rid_str.trim().parse().unwrap();
            if let Some(record) = do_read(&page, request_rid) {
                println!("{}", String::from_utf8(record).unwrap());
            }
        }

        if let Some(request_rid_str) = instruction.strip_prefix("delete ") {
            let request_rid = request_rid_str.trim().parse().unwrap();
            delete(&mut page, request_rid);
        }

        display_page(&page);
    }
}

fn do_fullscan(_page: &[u8]) {}

fn do_restart() -> [u8; PAGE_SIZE] {
    let mut empty_page = [0u8; PAGE_SIZE];

    // where to write next
    empty_page[POS_IS_THERE_SPACE] = 1;
    empty_page[POS_NEXT_ORDER] = 0;
    empty_page[POS_NUM_RECORDS] = 0;

    empty_page
}

fn get_record_offset(order: usize) -> usize {
    POS_START_RECORDS + RECORD_SIZE * order
}

fn get_order(rid: u8, page: &[u8]) -> usize {
    let (order, _) = page[BOUNDARY..=POS_START_OF_SLOTS]
        .chunks(2)
        .rev()
        .enumerate()
        .find(|(_, x)| x[0] == rid)
        .unwrap();
    order
}

fn get_next_empty_order(page: &[u8]) -> Option<usize> {
    let (order, _) = page[BOUNDARY..=POS_START_OF_SLOTS]
        .chunks(2)
        .rev()
        .enumerate()
        .find(|(_, x)| x[0] == POS_NULL)
        .unwrap();
    Some(order)
}

fn get_slot_offset(order: usize) -> usize {
    POS_START_OF_SLOTS - SLOT_SIZE * order
}

fn insert(page: &mut [u8], input: &str, rid: u8) {
    // crash if full
    if page[POS_IS_THERE_SPACE] == 0 {
        panic!("No space");
    }

    // record
    let record_offset = get_record_offset(page[POS_NEXT_ORDER] as usize);
    page[record_offset..record_offset + input.len()].copy_from_slice(input.as_bytes());

    // slots
    let slot_offset = get_slot_offset(page[POS_NEXT_ORDER] as usize);
    page[slot_offset - 1] = rid;
    page[slot_offset] = record_offset as u8;

    // update page metadata
    page[POS_NUM_RECORDS] += 1;
    if let Some(next_order) = get_next_empty_order(page) {
        page[POS_NEXT_ORDER] = next_order as u8;
        page[POS_IS_THERE_SPACE] = 1;
    } else {
        page[POS_NEXT_ORDER] = POS_NULL;
        page[POS_IS_THERE_SPACE] = 0;
    }

    // write back
    let mut file = File::create("hello").unwrap();
    file.write(&page).unwrap();

    println!("Written!")
}

fn do_read(page: &[u8], request_rid: u8) -> Option<Vec<u8>> {
    let order = get_order(request_rid, page);
    let record_offset = get_record_offset(order);
    get_record(record_offset, page)
}

fn get_record(offset: usize, page: &[u8]) -> Option<Vec<u8>> {
    let vvv = &page[offset..offset + RECORD_SIZE];
    Some(vvv.into())
}

/// when we delete a record, the next available space is just the space of the deleted record
fn delete(page: &mut [u8], request_rid: u8) {
    let order = get_order(request_rid, page);

    // record
    let record_offset = get_record_offset(order);
    page[record_offset..record_offset + RECORD_SIZE].copy_from_slice(&[0; RECORD_SIZE]);

    // slots
    let slot_offset = get_slot_offset(order);
    page[slot_offset] = POS_NULL;
    page[slot_offset - 1] = POS_NULL;

    let next_order = get_next_empty_order(page).unwrap();

    // update page metadata
    page[POS_NEXT_ORDER] = next_order as u8;
    page[POS_NUM_RECORDS] -= 1;
    page[POS_IS_THERE_SPACE] = 1;

    display_page(&page);

    // write back
    let mut file = File::create("hello").unwrap();
    file.write(&page).unwrap();
}

fn display_page(_page: &[u8]) {
    println!("\n---\nPage:");
    let output = Command::new("xxd")
        .arg("hello")
        .output()
        .expect("ls command failed to start");

    io::stdout().write_all(&output.stdout).unwrap();

    println!(
        "                                           |  | |
                                           |  | is there space?
                                           |  |  
                                           |  next order
                                           |
                                           total no. of records"
    );
}

#[cfg(test)]
mod tests {
    use crate::{
        delete, do_read, do_restart, insert, PAGE_SIZE, POS_START_OF_SLOTS, POS_START_RECORDS,
        RECORD_SIZE,
    };

    #[test]
    fn new_page() {
        let page = do_restart();
        assert_eq!(page.len(), PAGE_SIZE);
        assert_eq!(page[PAGE_SIZE - 1], 1);
    }

    #[test]
    fn insert_record() {
        let mut page = do_restart();

        let input = "hello";
        insert(&mut page, input, 0x1f);

        assert_eq!(page[POS_START_OF_SLOTS - 1], 0x1f);
        assert_eq!(
            &page[POS_START_RECORDS..POS_START_RECORDS + input.len()],
            input.as_bytes()
        );
    }

    #[test]
    fn delete_existing_record() {
        let mut page = do_restart();
        insert(&mut page, "aaaa", 0x01);
        insert(&mut page, "bbbb", 0x02);
        insert(&mut page, "cccc", 0x03);
        assert_eq!(page[POS_START_OF_SLOTS - 3], 0x02);
        assert_eq!(
            &page[POS_START_RECORDS + RECORD_SIZE..POS_START_RECORDS + RECORD_SIZE * 2],
            "bbbb\0\0\0\0".as_bytes()
        );

        delete(&mut page, 0x02);

        assert_eq!(page[POS_START_OF_SLOTS - 3], 0x00);
        assert_eq!(
            &page[POS_START_RECORDS + RECORD_SIZE..POS_START_RECORDS + RECORD_SIZE * 2],
            "\0\0\0\0\0\0\0\0".as_bytes()
        );
    }
}
