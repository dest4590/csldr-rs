#[derive(Clone, Copy)]
pub struct Pattern {
    pub bytes: &'static [u8],
    pub mask: &'static [u8],
}

impl Pattern {
    pub const fn new(bytes: &'static [u8], mask: &'static [u8]) -> Self {
        Self { bytes, mask }
    }

    pub unsafe fn scan_in_range(&self, start: usize, size: usize) -> Option<usize> {
        if size < self.bytes.len() {
            return None;
        }
        let end = start + size - self.bytes.len();
        let pat_len = self.bytes.len();

        for addr in start..=end {
            let mut matched = true;
            for i in 0..pat_len {
                if self.mask[i] == b'x' {
                    let b = *(addr as *const u8).add(i);
                    if b != self.bytes[i] {
                        matched = false;
                        break;
                    }
                }
            }
            if matched {
                return Some(addr);
            }
        }
        None
    }
}

pub unsafe fn find_function_by_string_xref(
    base: usize,
    image_size: usize,
    str_bytes: &[u8],
) -> Option<usize> {
    if image_size < str_bytes.len() + 4 {
        return None;
    }

    // 1. Scan for the string
    let mut str_addr = None;
    let end_str = base + image_size - str_bytes.len();
    for addr in base..=end_str {
        let slice = std::slice::from_raw_parts(addr as *const u8, str_bytes.len());
        if slice == str_bytes {
            str_addr = Some(addr);
            break;
        }
    }

    let str_va = str_addr?;

    // 2. Scan for push str_va (0x68 + 4-byte little endian pointer)
    let push_bytes = [
        0x68u8,
        (str_va & 0xFF) as u8,
        ((str_va >> 8) & 0xFF) as u8,
        ((str_va >> 16) & 0xFF) as u8,
        ((str_va >> 24) & 0xFF) as u8,
    ];

    let mut xref_addr = None;
    let end_xref = base + image_size - 5;
    for addr in base..=end_xref {
        let slice = std::slice::from_raw_parts(addr as *const u8, 5);
        if slice == push_bytes {
            xref_addr = Some(addr);
            break;
        }
    }

    let xref = xref_addr?;

    // 3. Scan backwards from xref to find standard function prologue
    let start_search = if xref > base + 2048 {
        xref - 2048
    } else {
        base
    };
    for addr in (start_search..xref).rev() {
        let b = std::slice::from_raw_parts(addr as *const u8, 3);
        // 55 8B EC (push ebp; mov ebp, esp) or 55 89 E5 or 83 EC 0C (sub esp, 0x0c)
        if b == [0x55, 0x8B, 0xEC] || b == [0x55, 0x89, 0xE5] {
            return Some(addr);
        }
    }

    None
}
