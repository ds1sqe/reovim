//! Selftests for retained memory/address-space diagnostics.

use {
    super::{
        ADDRESS_SPACE_STACK_FLAGS, ADDRESS_SPACE_TEXT_FLAGS, AddressSpaceImage,
        AddressSpaceObjectBacking, AddressSpaceObjectKind, AddressSpaceState,
        DEFAULT_USER_STACK_BYTES, EMPTY_ADDRESS_SPACE_OBJECT_RECORD,
        EMPTY_ADDRESS_SPACE_PAGE_RECORD, EMPTY_ADDRESS_SPACE_PAGE_TABLE_RECORD,
        EMPTY_ADDRESS_SPACE_RECORD, MAX_ADDRESS_SPACE_OBJECT_RECORDS,
        MAX_ADDRESS_SPACE_PAGE_RECORDS, MAX_ADDRESS_SPACE_PAGE_TABLE_RECORDS,
        MAX_ADDRESS_SPACE_RECORDS, USER_PAGE_BYTES, USER_STACK_TOP, USER_TEXT_BASE, address_space,
        allocate_process_address_space, install_shell_address_space, replace_process_address_space,
        reset_address_spaces, retire_process_address_space, snapshot_address_space_objects,
        snapshot_address_space_page_tables, snapshot_address_space_pages, snapshot_address_spaces,
    },
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(mm_tracks_address_space_lifecycle, {
    reset_address_spaces();

    let mut records = [EMPTY_ADDRESS_SPACE_RECORD; MAX_ADDRESS_SPACE_RECORDS];
    let count = snapshot_address_spaces(&mut records);
    testrt::check_eq(count, 2usize);
    let mut objects = [EMPTY_ADDRESS_SPACE_OBJECT_RECORD; MAX_ADDRESS_SPACE_OBJECT_RECORDS];
    testrt::check_eq(snapshot_address_space_objects(&mut objects), 0usize);
    let mut page_tables =
        [EMPTY_ADDRESS_SPACE_PAGE_TABLE_RECORD; MAX_ADDRESS_SPACE_PAGE_TABLE_RECORDS];
    testrt::check_eq(snapshot_address_space_page_tables(&mut page_tables), 0usize);
    let mut pages = [EMPTY_ADDRESS_SPACE_PAGE_RECORD; MAX_ADDRESS_SPACE_PAGE_RECORDS];
    testrt::check_eq(snapshot_address_space_pages(&mut pages), 0usize);

    let rootd = address_space(1).expect("rootd address space retained");
    testrt::check_eq(rootd.owner_pid, 1usize);
    testrt::check_eq(rootd.image_generation, 1usize);
    testrt::check_eq(rootd.state, AddressSpaceState::Active);
    testrt::check_eq(rootd.program_path, "rootd");
    testrt::check_eq(rootd.loader, "kernel");
    testrt::check_eq(rootd.entry_name, "rootd_main");
    testrt::check_eq(rootd.source_path, "rootd");
    testrt::check_eq(rootd.stack_bytes, 0usize);
    testrt::check_eq(rootd.region_count, 0usize);
    testrt::check_eq(rootd.text_start, 0usize);
    testrt::check_eq(rootd.text_end, 0usize);
    testrt::check_eq(rootd.text_flags, 0u32);
    testrt::check_eq(rootd.stack_start, 0usize);
    testrt::check_eq(rootd.stack_end, 0usize);
    testrt::check_eq(rootd.stack_flags, 0u32);

    install_shell_address_space("/bin/sh", "linked-bin", "bin_sh");
    let shell = address_space(2).expect("shell address space retained");
    testrt::check_eq(shell.owner_pid, 2usize);
    testrt::check_eq(shell.image_generation, 1usize);
    testrt::check_eq(shell.state, AddressSpaceState::Active);
    testrt::check_eq(shell.program_path, "/bin/sh");
    testrt::check_eq(shell.loader, "linked-bin");
    testrt::check_eq(shell.entry_name, "bin_sh");
    testrt::check_eq(shell.source_path, "/bin/sh");
    testrt::check_eq(shell.text_bytes, 0usize);
    testrt::check_eq(shell.text_checksum, 0u32);
    testrt::check_eq(shell.stack_bytes, DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(shell.region_count, 1usize);
    testrt::check_eq(shell.text_start, 0usize);
    testrt::check_eq(shell.text_end, 0usize);
    testrt::check_eq(shell.text_flags, 0u32);
    testrt::check_eq(shell.stack_start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(shell.stack_end, USER_STACK_TOP);
    testrt::check_eq(shell.stack_flags, ADDRESS_SPACE_STACK_FLAGS);
    testrt::check_eq(shell.page_table_id, 2usize);
    testrt::check_eq(shell.mapped_pages, 4usize);
    testrt::check_eq(shell.text_pages, 0usize);
    testrt::check_eq(shell.stack_pages, 4usize);
    let page_table_count = snapshot_address_space_page_tables(&mut page_tables);
    testrt::check_eq(page_table_count, 1usize);
    testrt::check_eq(page_tables[0].id, shell.page_table_id);
    testrt::check_eq(page_tables[0].address_space_id, 2usize);
    testrt::check_eq(page_tables[0].owner_pid, 2usize);
    testrt::check_eq(page_tables[0].state, AddressSpaceState::Active);
    testrt::check_eq(page_tables[0].root_table_id, shell.page_table_id);
    testrt::check_eq(page_tables[0].mapped_pages, 4usize);
    testrt::check_eq(page_tables[0].text_pages, 0usize);
    testrt::check_eq(page_tables[0].stack_pages, 4usize);
    testrt::check_eq(page_tables[0].text_flags, 0u32);
    testrt::check_eq(page_tables[0].stack_flags, ADDRESS_SPACE_STACK_FLAGS);
    let page_count = snapshot_address_space_pages(&mut pages);
    testrt::check_eq(page_count, 4usize);
    testrt::check_eq(pages[0].address_space_id, 2usize);
    testrt::check_eq(pages[0].page_table_id, shell.page_table_id);
    testrt::check_eq(pages[0].owner_pid, 2usize);
    testrt::check_eq(pages[0].state, AddressSpaceState::Active);
    testrt::check_eq(pages[0].kind, AddressSpaceObjectKind::Stack);
    testrt::check_eq(pages[0].backing, AddressSpaceObjectBacking::ZeroFill);
    testrt::check_eq(pages[0].page_index, 0usize);
    testrt::check_eq(pages[0].virtual_start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(
        pages[0].virtual_end,
        USER_STACK_TOP - DEFAULT_USER_STACK_BYTES + USER_PAGE_BYTES,
    );
    testrt::check_eq(pages[0].bytes, USER_PAGE_BYTES);
    testrt::check_eq(pages[0].source_offset, 0usize);
    testrt::check_eq(pages[0].flags, ADDRESS_SPACE_STACK_FLAGS);
    let object_count = snapshot_address_space_objects(&mut objects);
    testrt::check_eq(object_count, 1usize);
    testrt::check_eq(objects[0].address_space_id, 2usize);
    testrt::check_eq(objects[0].owner_pid, 2usize);
    testrt::check_eq(objects[0].state, AddressSpaceState::Active);
    testrt::check_eq(objects[0].kind, AddressSpaceObjectKind::Stack);
    testrt::check_eq(objects[0].backing, AddressSpaceObjectBacking::ZeroFill);
    testrt::check_eq(objects[0].start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(objects[0].end, USER_STACK_TOP);
    testrt::check_eq(objects[0].bytes, DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(objects[0].checksum, 0u32);
    testrt::check_eq(objects[0].flags, ADDRESS_SPACE_STACK_FLAGS);
    testrt::check_eq(objects[0].page_count, 4usize);

    let first = allocate_process_address_space(
        3,
        1,
        "/bin/pwd",
        "linked-bin",
        "bin_pwd",
        AddressSpaceImage::linked("/bin/pwd"),
    )
    .expect("process address space allocates");
    let current = address_space(first).expect("new address space retained");
    testrt::check_eq(current.owner_pid, 3usize);
    testrt::check_eq(current.image_generation, 1usize);
    testrt::check_eq(current.state, AddressSpaceState::Active);
    testrt::check_eq(current.program_path, "/bin/pwd");
    testrt::check_eq(current.source_path, "/bin/pwd");
    testrt::check_eq(current.text_bytes, 0usize);
    testrt::check_eq(current.text_checksum, 0u32);
    testrt::check_eq(current.stack_bytes, DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(current.region_count, 1usize);
    testrt::check_eq(current.text_start, 0usize);
    testrt::check_eq(current.text_end, 0usize);
    testrt::check_eq(current.stack_start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(current.stack_end, USER_STACK_TOP);
    testrt::check_eq(current.page_table_id, first);
    testrt::check_eq(current.mapped_pages, 4usize);
    testrt::check_eq(current.text_pages, 0usize);
    testrt::check_eq(current.stack_pages, 4usize);
    let page_table_count = snapshot_address_space_page_tables(&mut page_tables);
    testrt::check_eq(page_table_count, 2usize);
    testrt::check_eq(page_tables[1].id, first);
    testrt::check_eq(page_tables[1].address_space_id, first);
    testrt::check_eq(page_tables[1].owner_pid, 3usize);
    testrt::check_eq(page_tables[1].mapped_pages, 4usize);
    let page_count = snapshot_address_space_pages(&mut pages);
    testrt::check_eq(page_count, 8usize);
    testrt::check_eq(pages[4].address_space_id, first);
    testrt::check_eq(pages[4].page_table_id, first);
    testrt::check_eq(pages[4].owner_pid, 3usize);
    testrt::check_eq(pages[4].state, AddressSpaceState::Active);
    testrt::check_eq(pages[4].kind, AddressSpaceObjectKind::Stack);
    testrt::check_eq(pages[4].page_index, 0usize);
    testrt::check_eq(pages[4].virtual_start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(pages[4].bytes, USER_PAGE_BYTES);
    testrt::check_eq(pages[7].page_index, 3usize);
    testrt::check_eq(pages[7].virtual_end, USER_STACK_TOP);
    let object_count = snapshot_address_space_objects(&mut objects);
    testrt::check_eq(object_count, 2usize);
    testrt::check_eq(objects[1].address_space_id, first);
    testrt::check_eq(objects[1].kind, AddressSpaceObjectKind::Stack);
    testrt::check_eq(objects[1].backing, AddressSpaceObjectBacking::ZeroFill);
    testrt::check_eq(objects[1].state, AddressSpaceState::Active);

    let second = replace_process_address_space(
        3,
        first,
        2,
        "/bin/cat",
        "source-image",
        "bin_cat",
        AddressSpaceImage::source("/bin/cat", 37, 0x1234),
    )
    .expect("replacement address space allocates");
    let old = address_space(first).expect("old address space remains diagnostic evidence");
    testrt::check_eq(old.owner_pid, 3usize);
    testrt::check_eq(old.image_generation, 1usize);
    testrt::check_eq(old.state, AddressSpaceState::Retired);
    testrt::check_eq(old.program_path, "/bin/pwd");

    let replacement = address_space(second).expect("replacement address space retained");
    testrt::check_eq(replacement.owner_pid, 3usize);
    testrt::check_eq(replacement.image_generation, 2usize);
    testrt::check_eq(replacement.state, AddressSpaceState::Active);
    testrt::check_eq(replacement.program_path, "/bin/cat");
    testrt::check_eq(replacement.loader, "source-image");
    testrt::check_eq(replacement.entry_name, "bin_cat");
    testrt::check_eq(replacement.source_path, "/bin/cat");
    testrt::check_eq(replacement.text_bytes, 37usize);
    testrt::check_eq(replacement.text_checksum, 0x1234);
    testrt::check_eq(replacement.stack_bytes, DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(replacement.region_count, 2usize);
    testrt::check_eq(replacement.text_start, USER_TEXT_BASE);
    testrt::check_eq(replacement.text_end, USER_TEXT_BASE + USER_PAGE_BYTES);
    testrt::check_eq(replacement.text_flags, ADDRESS_SPACE_TEXT_FLAGS);
    testrt::check_eq(replacement.stack_start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(replacement.stack_end, USER_STACK_TOP);
    testrt::check_eq(replacement.stack_flags, ADDRESS_SPACE_STACK_FLAGS);
    testrt::check_eq(replacement.page_table_id, second);
    testrt::check_eq(replacement.mapped_pages, 5usize);
    testrt::check_eq(replacement.text_pages, 1usize);
    testrt::check_eq(replacement.stack_pages, 4usize);
    let page_table_count = snapshot_address_space_page_tables(&mut page_tables);
    testrt::check_eq(page_table_count, 3usize);
    testrt::check_eq(page_tables[1].address_space_id, first);
    testrt::check_eq(page_tables[1].state, AddressSpaceState::Retired);
    testrt::check_eq(page_tables[2].id, second);
    testrt::check_eq(page_tables[2].address_space_id, second);
    testrt::check_eq(page_tables[2].owner_pid, 3usize);
    testrt::check_eq(page_tables[2].state, AddressSpaceState::Active);
    testrt::check_eq(page_tables[2].root_table_id, second);
    testrt::check_eq(page_tables[2].mapped_pages, 5usize);
    testrt::check_eq(page_tables[2].text_pages, 1usize);
    testrt::check_eq(page_tables[2].stack_pages, 4usize);
    testrt::check_eq(page_tables[2].text_flags, ADDRESS_SPACE_TEXT_FLAGS);
    testrt::check_eq(page_tables[2].stack_flags, ADDRESS_SPACE_STACK_FLAGS);
    let page_count = snapshot_address_space_pages(&mut pages);
    testrt::check_eq(page_count, 13usize);
    testrt::check_eq(pages[4].address_space_id, first);
    testrt::check_eq(pages[4].state, AddressSpaceState::Retired);
    testrt::check_eq(pages[8].address_space_id, second);
    testrt::check_eq(pages[8].page_table_id, second);
    testrt::check_eq(pages[8].owner_pid, 3usize);
    testrt::check_eq(pages[8].state, AddressSpaceState::Active);
    testrt::check_eq(pages[8].kind, AddressSpaceObjectKind::Text);
    testrt::check_eq(pages[8].backing, AddressSpaceObjectBacking::SourceText);
    testrt::check_eq(pages[8].source_path, "/bin/cat");
    testrt::check_eq(pages[8].page_index, 0usize);
    testrt::check_eq(pages[8].virtual_start, USER_TEXT_BASE);
    testrt::check_eq(pages[8].virtual_end, USER_TEXT_BASE + USER_PAGE_BYTES);
    testrt::check_eq(pages[8].bytes, 37usize);
    testrt::check_eq(pages[8].source_offset, 0usize);
    testrt::check_eq(pages[8].flags, ADDRESS_SPACE_TEXT_FLAGS);
    testrt::check_eq(pages[9].address_space_id, second);
    testrt::check_eq(pages[9].kind, AddressSpaceObjectKind::Stack);
    testrt::check_eq(pages[9].page_index, 1usize);
    testrt::check_eq(pages[9].virtual_start, USER_STACK_TOP - DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(pages[9].bytes, USER_PAGE_BYTES);
    testrt::check_eq(pages[12].page_index, 4usize);
    testrt::check_eq(pages[12].virtual_end, USER_STACK_TOP);
    let object_count = snapshot_address_space_objects(&mut objects);
    testrt::check_eq(object_count, 4usize);
    testrt::check_eq(objects[1].address_space_id, first);
    testrt::check_eq(objects[1].kind, AddressSpaceObjectKind::Stack);
    testrt::check_eq(objects[1].state, AddressSpaceState::Retired);
    testrt::check_eq(objects[2].address_space_id, second);
    testrt::check_eq(objects[2].kind, AddressSpaceObjectKind::Text);
    testrt::check_eq(objects[2].backing, AddressSpaceObjectBacking::SourceText);
    testrt::check_eq(objects[2].source_path, "/bin/cat");
    testrt::check_eq(objects[2].start, USER_TEXT_BASE);
    testrt::check_eq(objects[2].end, USER_TEXT_BASE + USER_PAGE_BYTES);
    testrt::check_eq(objects[2].bytes, 37usize);
    testrt::check_eq(objects[2].checksum, 0x1234);
    testrt::check_eq(objects[2].flags, ADDRESS_SPACE_TEXT_FLAGS);
    testrt::check_eq(objects[2].page_count, 1usize);
    testrt::check_eq(objects[3].address_space_id, second);
    testrt::check_eq(objects[3].kind, AddressSpaceObjectKind::Stack);
    testrt::check_eq(objects[3].backing, AddressSpaceObjectBacking::ZeroFill);
    testrt::check_eq(objects[3].state, AddressSpaceState::Active);
    testrt::check_eq(objects[3].page_count, 4usize);

    retire_process_address_space(3, second);
    let retired = address_space(second).expect("replacement retire remains visible");
    testrt::check_eq(retired.state, AddressSpaceState::Retired);
    let page_table_count = snapshot_address_space_page_tables(&mut page_tables);
    testrt::check_eq(page_table_count, 3usize);
    testrt::check_eq(page_tables[2].state, AddressSpaceState::Retired);
    let page_count = snapshot_address_space_pages(&mut pages);
    testrt::check_eq(page_count, 13usize);
    testrt::check_eq(pages[8].state, AddressSpaceState::Retired);
    testrt::check_eq(pages[12].state, AddressSpaceState::Retired);
    let object_count = snapshot_address_space_objects(&mut objects);
    testrt::check_eq(object_count, 4usize);
    testrt::check_eq(objects[2].state, AddressSpaceState::Retired);
    testrt::check_eq(objects[3].state, AddressSpaceState::Retired);
});
